use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, ensure};
use directories::ProjectDirs;
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha1::Digest;
use url::Url;

use crate::client::{Api, host_url};
use crate::config::Config;
use crate::{auth, document};

#[derive(Clone, Copy, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Governance,
    Infrastructure,
    Changelog,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Governance => "governance",
            Self::Infrastructure => "infrastructure",
            Self::Changelog => "changelog",
        }
    }
    fn includes(self, path: &str) -> bool {
        match self {
            Self::Changelog => {
                matches!(path, "kennel.md" | "governance.md" | "infrastructure.md")
                    || path
                        .strip_prefix("updates/")
                        .is_some_and(|name| crate::changelog::update_number(name).is_some())
            }
            Self::Governance => {
                path == "data/org.toml"
                    || path
                        .strip_prefix("data/teams/")
                        .is_some_and(|name| !name.contains('/') && name.ends_with(".toml"))
            }
            Self::Infrastructure => {
                path == "flake.nix"
                    || (path.ends_with(".nix")
                        && (path.starts_with("modules/hosts/")
                            || path.starts_with("modules/roles/")))
            }
        }
    }
}

pub struct Sources<'a> {
    pub config: &'a Config,
    pub host: Option<&'a str>,
    pub cache: Option<&'a Path>,
}

pub struct Source {
    pub directory: PathBuf,
    pub revision: Option<String>,
    info: Value,
}

impl Source {
    pub fn attach(&self, value: &impl Serialize) -> Result<Value> {
        let mut value = serde_json::to_value(value)?;
        let objects: Vec<&mut Value> = match &mut value {
            Value::Array(items) => items.iter_mut().collect(),
            value => vec![value],
        };
        for object in objects {
            object
                .as_object_mut()
                .context("source output must contain objects")?
                .insert("dataSource".into(), self.info.clone());
        }
        Ok(value)
    }
    pub fn describe(&self) -> String {
        match &self.revision {
            Some(revision) => format!(
                "Canonical snapshot {revision}; offline: {}",
                self.info["offline"]
            ),
            None => format!(
                "Local working tree {}; remote freshness is not verified",
                self.directory.display()
            ),
        }
    }
}

impl Sources<'_> {
    pub fn load(&self, kind: Kind, checkout: Option<PathBuf>, offline: bool) -> Result<Source> {
        let origin = host_url(self.host.unwrap_or(self.config.host()))?;
        self.load_at(kind, checkout, offline, &origin)
    }

    pub fn load_at(
        &self,
        kind: Kind,
        checkout: Option<PathBuf>,
        offline: bool,
        origin: &Url,
    ) -> Result<Source> {
        if let Some(directory) = checkout {
            return Ok(Source {
                info: json!({"kind": "checkout", "path": directory, "revision": null, "offline": offline}),
                directory,
                revision: None,
            });
        }
        let cache = match self.cache {
            Some(path) => path.to_owned(),
            None => ProjectDirs::from("", "", "collar")
                .context("cannot find cache directory; use --cache-dir PATH")?
                .cache_dir()
                .to_owned(),
        };
        let mut cache = cache.join("sources").join("v1");
        let encoded: String = origin
            .as_str()
            .bytes()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        for part in encoded.as_bytes().chunks(64) {
            cache.push(std::str::from_utf8(part)?);
        }
        let cache = cache.join(kind.name());
        let revision = if offline {
            let revision = std::fs::read_to_string(cache.join("current")).with_context(|| {
                format!(
                    "no readable cached {} snapshot; run without --offline to refresh",
                    kind.name()
                )
            })?;
            revision.trim().to_owned()
        } else {
            let api = auth::api(origin.clone(), self.config)?;
            let revision = head(&api, kind)?;
            valid_revision(&revision)?;
            if !cache.join(&revision).exists() {
                download(&api, kind, &revision, &cache)?;
            }
            validate_snapshot(&cache.join(&revision), kind, &revision, origin)?;
            let mut pointer =
                tempfile::NamedTempFile::new_in(&cache).context("create snapshot pointer")?;
            pointer
                .write_all(revision.as_bytes())
                .context("write snapshot pointer")?;
            pointer
                .as_file()
                .sync_all()
                .context("sync snapshot pointer")?;
            pointer
                .persist(cache.join("current"))
                .context("publish snapshot pointer")?;
            revision
        };
        valid_revision(&revision)?;
        let directory = cache.join(&revision);
        let manifest = validate_snapshot(&directory, kind, &revision, origin)?;
        Ok(Source {
            info: json!({"kind": "snapshot", "repository": format!("ScottyLabs/{}", kind.name()), "host": origin,
            "revision": revision, "downloadedAt": manifest.downloaded_at, "offline": offline}),
            directory,
            revision: Some(revision),
        })
    }
}

fn valid_revision(revision: &str) -> Result<()> {
    ensure!(
        matches!(revision.len(), 40 | 64) && revision.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "source revision must be a full Git object ID"
    );
    Ok(())
}

fn valid_path(path: &str) -> Result<()> {
    ensure!(
        !path.contains(['\\', '\0'])
            && !path.chars().any(char::is_control)
            && path
                .split('/')
                .all(|part| !part.is_empty() && part != "." && part != ".." && !part.contains(':')),
        "source tree contains an unsafe path"
    );
    Ok(())
}

#[derive(Deserialize)]
struct Repository {
    default_branch: String,
}
#[derive(Deserialize)]
struct Branch {
    commit: Commit,
}
#[derive(Deserialize)]
struct Commit {
    id: String,
}

fn head(api: &Api, kind: Kind) -> Result<String> {
    let repository: Repository = api.json(
        Method::GET,
        &["repos", "ScottyLabs", kind.name()],
        &[],
        None,
    )?;
    ensure!(
        !repository.default_branch.is_empty(),
        "canonical repository has no default branch"
    );
    let branch: Branch = api.json(
        Method::GET,
        &[
            "repos",
            "ScottyLabs",
            kind.name(),
            "branches",
            &repository.default_branch,
        ],
        &[],
        None,
    )?;
    Ok(branch.commit.id)
}

#[derive(Deserialize)]
struct Tree {
    page: u64,
    total_count: usize,
    truncated: bool,
    tree: Vec<Entry>,
}
#[derive(Deserialize)]
struct Entry {
    path: String,
    sha: String,
    mode: String,
    #[serde(rename = "type")]
    kind: String,
    size: u64,
}
#[derive(Serialize, Deserialize)]
struct File {
    path: String,
    sha: String,
    size: u64,
}
#[derive(Serialize, Deserialize)]
struct Manifest {
    kind: Kind,
    host: String,
    revision: String,
    downloaded_at: u64,
    files: Vec<File>,
}

fn download(api: &Api, kind: Kind, revision: &str, cache: &Path) -> Result<()> {
    let mut paths = BTreeSet::new();
    let mut selected = Vec::new();
    let mut total = None;
    let mut complete = false;
    let mut bytes = 0;
    for page in 1..=100 {
        let tree: Tree = api.json(
            Method::GET,
            &["repos", "ScottyLabs", kind.name(), "git", "trees", revision],
            &[
                ("recursive".into(), "true".into()),
                ("page".into(), page.to_string()),
                ("per_page".into(), "1000".into()),
            ],
            None,
        )?;
        ensure!(
            tree.page == page && total.is_none_or(|count| count == tree.total_count),
            "source tree pagination changed unexpectedly"
        );
        total = Some(tree.total_count);
        let empty = tree.tree.is_empty();
        for entry in tree.tree {
            valid_path(&entry.path)?;
            ensure!(
                paths.insert(entry.path.clone()),
                "source tree contains duplicate paths"
            );
            ensure!(paths.len() <= 100_000, "source tree exceeds 100000 entries");
            if entry.kind == "blob" && kind.includes(&entry.path) {
                ensure!(
                    matches!(entry.mode.as_str(), "100644" | "100755"),
                    "source snapshots do not support symlinks"
                );
                ensure!(entry.size <= 8 * 1024 * 1024, "source file exceeds 8 MiB");
                bytes += entry.size;
                ensure!(
                    entry.size <= 8 * 1024 * 1024
                        && bytes <= 32 * 1024 * 1024
                        && selected.len() < 500,
                    "source snapshot exceeds file or size bounds"
                );
                selected.push(File {
                    path: entry.path,
                    sha: entry.sha,
                    size: entry.size,
                });
            }
        }
        if !tree.truncated {
            ensure!(paths.len() == tree.total_count, "source tree is incomplete");
            complete = true;
            break;
        }
        ensure!(!empty, "source tree pagination did not advance");
    }
    ensure!(complete, "source tree exceeded 100 pages");
    ensure!(
        !selected.is_empty(),
        "canonical source has no required files"
    );
    std::fs::create_dir_all(cache).context("create source cache directory")?;
    let staging =
        tempfile::tempdir_in(cache).context("create source snapshot staging directory")?;
    for file in &selected {
        let mut path = vec!["repos", "ScottyLabs", kind.name(), "raw"];
        path.extend(file.path.split('/'));
        let content = api.text(Method::GET, &path, &[("ref".into(), revision.into())], None)?;
        ensure!(
            content.len() as u64 == file.size,
            "source file size disagrees with pinned tree"
        );
        verify_blob(content.as_bytes(), &file.sha)?;
        let destination = staging.path().join(&file.path);
        std::fs::create_dir_all(destination.parent().context("source file has no parent")?)?;
        std::fs::write(destination, content).context("write source file")?;
    }
    let manifest = Manifest {
        kind,
        host: api.origin().to_string(),
        revision: revision.into(),
        downloaded_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("system clock is before the Unix epoch")?
            .as_secs(),
        files: selected,
    };
    std::fs::write(
        staging.path().join("manifest.toml"),
        toml::to_string(&manifest)?,
    )
    .context("write source manifest")?;
    validate_snapshot(staging.path(), kind, revision, api.origin())?;
    std::fs::rename(staging.path(), cache.join(revision)).context(
        "publish source snapshot; a concurrent refresh may have completed, retry the command",
    )?;
    Ok(())
}

fn validate_snapshot(
    directory: &Path,
    kind: Kind,
    revision: &str,
    origin: &Url,
) -> Result<Manifest> {
    ensure!(
        std::fs::symlink_metadata(directory)?.is_dir()
            && std::fs::symlink_metadata(directory.join("manifest.toml"))?.is_file(),
        "cached snapshot directory or manifest is invalid"
    );
    let manifest: Manifest = document::parse(
        &std::fs::read_to_string(directory.join("manifest.toml"))
            .context("read source manifest")?,
        "source manifest",
    )?;
    ensure!(
        manifest.kind == kind && manifest.revision == revision && manifest.host == origin.as_str(),
        "cached source identity does not match the requested repository"
    );
    let mut paths = BTreeSet::new();
    ensure!(
        !manifest.files.is_empty() && manifest.files.len() <= 500,
        "cached source manifest has an invalid file count"
    );
    let mut bytes = 0u64;
    for file in &manifest.files {
        valid_path(&file.path)?;
        ensure!(
            file.size <= 8 * 1024 * 1024,
            "cached source file exceeds 8 MiB"
        );
        bytes += file.size;
        ensure!(bytes <= 32 * 1024 * 1024, "cached source exceeds 32 MiB");
        let mut parent = directory.to_owned();
        let parts: Vec<_> = file.path.split('/').collect();
        for part in &parts[..parts.len() - 1] {
            parent.push(part);
            ensure!(
                std::fs::symlink_metadata(&parent)?.is_dir(),
                "cached source parent is not a directory"
            );
        }
        ensure!(
            kind.includes(&file.path) && paths.insert(&file.path),
            "cached source manifest contains an invalid or duplicate path"
        );
        let metadata = std::fs::symlink_metadata(directory.join(&file.path))
            .context("cached source is incomplete; remove the snapshot directory and refresh")?;
        ensure!(
            metadata.is_file() && metadata.len() == file.size,
            "cached source file is invalid; remove the snapshot directory and refresh"
        );
        verify_blob(
            &std::fs::read(directory.join(&file.path)).context("read cached source")?,
            &file.sha,
        )?;
    }
    match kind {
        Kind::Governance => {
            ensure!(
                crate::project::organization_origin(directory)? == *origin,
                "governance snapshot declares a different forge"
            );
            crate::governance::projects(directory)?;
        }
        Kind::Infrastructure => {
            ensure!(
                directory.join("flake.nix").is_file(),
                "infrastructure snapshot is missing flake.nix"
            );
            crate::infra::hosts(directory)?;
        }
        Kind::Changelog => {
            for name in ["kennel.md", "governance.md", "infrastructure.md"] {
                ensure!(
                    directory.join(name).is_file(),
                    "changelog snapshot is missing {name}"
                );
            }
        }
    }
    Ok(manifest)
}

fn verify_blob(content: &[u8], expected: &str) -> Result<()> {
    valid_revision(expected)?;
    let header = format!("blob {}\0", content.len());
    let actual = if expected.len() == 40 {
        let mut hash = sha1::Sha1::new();
        hash.update(header.as_bytes());
        hash.update(content);
        format!("{:x}", hash.finalize())
    } else {
        let mut hash = sha2::Sha256::new();
        hash.update(header.as_bytes());
        hash.update(content);
        format!("{:x}", hash.finalize())
    };
    ensure!(
        actual.eq_ignore_ascii_case(expected),
        "source content does not match its pinned Git blob; remove the snapshot directory and refresh"
    );
    Ok(())
}
