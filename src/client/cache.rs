use super::{Api, BODY_LIMIT, read_body};
use anyhow::{Context, Result};
use directories::ProjectDirs;
use reqwest::{Method, StatusCode, header};
use serde::{Deserialize, Serialize, de::IgnoredAny};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Write},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    time::SystemTime,
};

const ENTRY_LIMIT: u64 = BODY_LIMIT * 2 + 4096;
const MAX_ENTRIES: usize = 256;
const MAX_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Deserialize, Serialize)]
struct Entry {
    body: String,
    etag: Option<String>,
    modified: Option<String>,
}

impl Api {
    fn cache_partition(&self) -> Option<PathBuf> {
        let root = self.cache.directory.clone().or_else(|| {
            ProjectDirs::from("", "", "collar").map(|dirs| dirs.cache_dir().to_owned())
        })?;
        let identity = serde_json::to_vec(&(self.origin.as_str(), self.token.as_deref())).ok()?;
        Some(
            root.join("api")
                .join("v2")
                .join(format!("{:x}", Sha256::digest(identity))),
        )
    }

    pub(super) fn invalidate_read_cache(&self) {
        if let Some(directory) = self.cache_partition() {
            let _ = fs::remove_dir_all(directory);
        }
    }

    pub(super) fn read_text(&self, path: &[&str], query: &[(String, String)]) -> Result<String> {
        let partition = self.cache_partition();
        let key = serde_json::to_vec(&(path, query))?;
        let file = partition
            .as_ref()
            .map(|dir| dir.join(format!("{:x}.json", Sha256::digest(key))));
        let cached = if self.cache.refresh {
            None
        } else {
            file.as_deref().and_then(load_entry)
        };
        let mut headers = Vec::new();
        if let Some(entry) = &cached {
            if let Some(etag) = &entry.etag {
                headers.push(("If-None-Match", etag.as_str()));
            } else if let Some(modified) = &entry.modified {
                headers.push(("If-Modified-Since", modified.as_str()));
            }
        }
        let response = self.request_headers(Method::GET, path, query, None, &headers)?;
        let no_store = response
            .headers()
            .get_all(header::CACHE_CONTROL)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .flat_map(|value| value.split(','))
            .any(|directive| directive.trim().eq_ignore_ascii_case("no-store"));
        let etag = response
            .headers()
            .get(header::ETAG)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let modified = response
            .headers()
            .get(header::LAST_MODIFIED)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);

        let (entry, changed) = if response.status() == StatusCode::NOT_MODIFIED {
            let mut entry = cached.context("Forge returned 304 without a cached response")?;
            let changed = etag
                .as_ref()
                .is_some_and(|value| entry.etag.as_ref() != Some(value))
                || modified
                    .as_ref()
                    .is_some_and(|value| entry.modified.as_ref() != Some(value));
            entry.etag = etag.or(entry.etag);
            entry.modified = modified.or(entry.modified);
            (entry, changed)
        } else {
            (
                Entry {
                    body: read_body(response)?,
                    etag,
                    modified,
                },
                true,
            )
        };
        if let (Some(partition), Some(file)) = (partition, file) {
            if no_store {
                let _ = fs::remove_file(file);
            } else if changed && serde_json::from_str::<IgnoredAny>(&entry.body).is_ok() {
                // Cache storage is optional; a failed cache write must not fail the read.
                let _ = save_entry(&partition, &file, &entry);
            }
        }
        Ok(entry.body)
    }
}

fn load_entry(path: &Path) -> Option<Entry> {
    let mut bytes = Vec::new();
    File::open(path)
        .ok()?
        .take(ENTRY_LIMIT + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() as u64 > ENTRY_LIMIT {
        return None;
    }
    let entry: Entry = serde_json::from_slice(&bytes).ok()?;
    (entry.body.len() as u64 <= BODY_LIMIT
        && serde_json::from_str::<IgnoredAny>(&entry.body).is_ok())
    .then_some(entry)
}

fn save_entry(directory: &Path, file: &Path, entry: &Entry) -> Result<()> {
    fs::create_dir_all(directory)?;
    fs::set_permissions(directory, fs::Permissions::from_mode(0o700))?;
    let mut temporary = tempfile::NamedTempFile::new_in(directory)?;
    temporary.write_all(&serde_json::to_vec(entry)?)?;
    temporary.persist(file)?;
    if let Some(root) = directory.parent() {
        prune_cache(root, file, MAX_ENTRIES, MAX_BYTES);
    }
    Ok(())
}

fn managed_name(name: &std::ffi::OsStr, suffix: &str) -> bool {
    name.to_str()
        .and_then(|name| name.strip_suffix(suffix))
        .is_some_and(|name| name.len() == 64 && name.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn prune_cache(root: &Path, keep: &Path, max_entries: usize, max_bytes: u64) {
    let Ok(partitions) = fs::read_dir(root) else {
        return;
    };
    let mut files = Vec::new();
    let mut directories = Vec::new();
    for partition in partitions.flatten() {
        if !managed_name(&partition.file_name(), "")
            || !partition.file_type().is_ok_and(|kind| kind.is_dir())
        {
            continue;
        }
        let Ok(entries) = fs::read_dir(partition.path()) else {
            continue;
        };
        directories.push(partition.path());
        for entry in entries.flatten() {
            if !managed_name(&entry.file_name(), ".json")
                || !entry.file_type().is_ok_and(|kind| kind.is_file())
            {
                continue;
            }
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            files.push((
                entry.path(),
                metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                metadata.len(),
            ));
        }
    }
    files.sort_by_key(|entry| std::cmp::Reverse((entry.0 == keep, entry.1)));
    let mut bytes = 0u64;
    for (index, (path, _, length)) in files.into_iter().enumerate() {
        if index >= max_entries || bytes.saturating_add(length) > max_bytes {
            let _ = fs::remove_file(path);
        } else {
            bytes = bytes.saturating_add(length);
        }
    }
    for directory in directories {
        let _ = fs::remove_dir(directory);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn eviction_is_bounded_keeps_the_updated_entry_and_ignores_unmanaged_paths() {
        let temp = tempfile::tempdir().unwrap();
        let partition = temp.path().join("a".repeat(64));
        fs::create_dir(&partition).unwrap();
        let mut paths = Vec::new();
        for index in 0..4 {
            let path = partition.join(format!("{index:064x}.json"));
            fs::write(&path, [b'x'; 10]).unwrap();
            paths.push(path);
        }
        let note = partition.join("notes.txt");
        fs::write(&note, b"retained").unwrap();
        let outside = tempfile::tempdir().unwrap();
        let outside_file = outside.path().join(format!("{:064x}.json", 0));
        fs::write(&outside_file, b"retained").unwrap();
        std::os::unix::fs::symlink(outside.path(), temp.path().join("b".repeat(64))).unwrap();
        prune_cache(temp.path(), &paths[0], 2, 20);
        assert!(paths[0].exists());
        assert_eq!(paths.iter().filter(|path| path.exists()).count(), 2);
        assert!(note.exists());
        assert!(outside_file.exists());
    }
}
