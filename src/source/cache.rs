//! Retain the current snapshot and serialize readers with cache maintenance.

use std::fs::{self, File, OpenOptions};
use std::path::Path;

use anyhow::{Context, Result};

use super::valid_revision;

pub(super) fn lock(directory: &Path, offline: bool) -> Result<File> {
    if !offline {
        fs::create_dir_all(directory).context("create source cache directory")?;
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(directory.join(".lock"))
        .context("open source cache lock")?;
    file.lock().context("lock source cache")?;
    Ok(file)
}

// Hold the source lock and validate the current snapshot before calling this.
pub(super) fn evict(directory: &Path, current: &str) -> Result<()> {
    for entry in fs::read_dir(directory).context("read source cache for eviction")? {
        let entry = entry.context("read source cache entry")?;
        let name = entry.file_name();
        let Some(revision) = name.to_str() else {
            continue;
        };
        if revision != current && valid_revision(revision).is_ok() && entry.file_type()?.is_dir() {
            fs::remove_dir_all(entry.path()).context("evict obsolete source snapshot")?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eviction_keeps_current_and_ignores_unmanaged_entries() {
        let temp = tempfile::tempdir().unwrap();
        let current = "1".repeat(40);
        let obsolete = "2".repeat(40);
        for name in [&current, &obsolete, "unmanaged"] {
            fs::create_dir(temp.path().join(name)).unwrap();
        }
        let _guard = lock(temp.path(), false).unwrap();
        evict(temp.path(), &current).unwrap();
        assert!(temp.path().join(&current).is_dir());
        assert!(!temp.path().join(&obsolete).exists());
        assert!(temp.path().join("unmanaged").is_dir());
    }

    #[test]
    fn active_reader_blocks_maintenance_until_released() {
        let temp = tempfile::tempdir().unwrap();
        let guard = lock(temp.path(), false).unwrap();
        let contender = OpenOptions::new()
            .read(true)
            .write(true)
            .open(temp.path().join(".lock"))
            .unwrap();
        assert!(contender.try_lock().is_err());
        drop(guard);
        assert!(contender.try_lock().is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn eviction_does_not_follow_snapshot_symlinks() {
        let temp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("keep"), "data").unwrap();
        std::os::unix::fs::symlink(outside.path(), temp.path().join("2".repeat(40))).unwrap();
        let _guard = lock(temp.path(), false).unwrap();
        evict(temp.path(), &"1".repeat(40)).unwrap();
        assert!(outside.path().join("keep").is_file());
    }
}
