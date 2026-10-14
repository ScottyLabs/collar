use anyhow::{Context, Result, bail, ensure};
use std::process::Command;
use url::Url;

use crate::client::{DEFAULT_HOST, host_url};

#[derive(Debug)]
pub struct Repo {
    pub origin: Url,
    pub owner: String,
    pub name: String,
}

impl Repo {
    pub fn parse(value: &str, host: Option<&str>) -> Result<Self> {
        let parts: Vec<_> = value.split('/').collect();
        let (repo_host, owner, name) = match parts.as_slice() {
            [owner, name] => (host.unwrap_or(DEFAULT_HOST), *owner, *name),
            [repo_host, owner, name] => (*repo_host, *owner, *name),
            _ => bail!("repository must be [HOST/]OWNER/REPO"),
        };
        let origin = host_url(repo_host)?;
        if let Some(host) = host {
            ensure!(
                origin == host_url(host)?,
                "repository host disagrees with --host"
            );
        }
        let name = name.strip_suffix(".git").unwrap_or(name);
        for part in [owner, name] {
            ensure!(
                !part.is_empty()
                    && part != "."
                    && part != ".."
                    && part
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')),
                "invalid repository owner or name"
            );
        }
        Ok(Self {
            origin,
            owner: owner.into(),
            name: name.into(),
        })
    }

    pub fn remote(value: &str, host: Option<&str>) -> Result<Self> {
        let url = if value.contains("://") {
            Url::parse(value).context("invalid Git remote URL")?
        } else {
            let (authority, path) = value.split_once(':').context("invalid Git remote URL")?;
            Url::parse(&format!("ssh://{authority}/{path}")).context("invalid SSH remote")?
        };
        ensure!(
            matches!(url.scheme(), "https" | "http" | "ssh"),
            "unsupported Git remote protocol"
        );
        let remote_host = url.host_str().context("Git remote has no host")?;
        let api_host = match url.scheme() {
            "ssh" => remote_host.to_owned(),
            _ => format!(
                "{}://{}",
                url.scheme(),
                &url[url::Position::BeforeHost..url::Position::AfterPort]
            ),
        };
        Self::parse(url.path().trim_matches('/'), Some(&api_host)).and_then(|repo| {
            if let Some(host) = host {
                ensure!(
                    repo.origin == host_url(host)?,
                    "Git remote host disagrees with --host"
                );
            }
            Ok(repo)
        })
    }

    pub fn resolve(explicit: Option<&str>, host: Option<&str>) -> Result<Self> {
        if let Some(value) = explicit {
            return Self::parse(value, host);
        }
        let names = Command::new("git")
            .arg("remote")
            .output()
            .context("run git remote")?;
        ensure!(
            names.status.success(),
            "git remote failed ({}); inspect the Git checkout or specify -R OWNER/REPO",
            names.status
        );
        let names = std::str::from_utf8(&names.stdout).context("Git remote names are not UTF-8")?;
        let remote = ["upstream", "origin"]
            .into_iter()
            .find(|remote| names.lines().any(|name| name == *remote))
            .context("no upstream or origin remote; specify -R OWNER/REPO")?;
        let output = Command::new("git")
            .args(["remote", "get-url", remote])
            .output()
            .with_context(|| format!("read Git remote {remote}"))?;
        ensure!(
            output.status.success(),
            "git could not read configured remote {remote} ({}); inspect its URL configuration",
            output.status
        );
        Self::remote(
            std::str::from_utf8(&output.stdout)
                .context("Git remote URL is not UTF-8")?
                .trim(),
            host,
        )
        .with_context(|| format!("parse configured remote {remote}"))
    }

    pub fn path(&self) -> [&str; 3] {
        ["repos", &self.owner, &self.name]
    }
}

pub fn branch() -> Result<String> {
    let output = Command::new("git")
        .args(["symbolic-ref", "--quiet", "--short", "HEAD"])
        .output()
        .context("read current Git branch")?;
    ensure!(
        output.status.success(),
        "no current Git branch; specify a PR number or URL"
    );
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_scottylabs_ssh_and_https_remotes() {
        for remote in [
            "ssh://forgejo@git.cmu.dev/ScottyLabs/terrier.git",
            "forgejo@git.cmu.dev:ScottyLabs/terrier.git",
            "https://git.cmu.dev/ScottyLabs/terrier.git",
        ] {
            let repo = Repo::remote(remote, None).unwrap();
            assert_eq!(repo.origin.as_str(), "https://git.cmu.dev/");
            assert_eq!(repo.owner, "ScottyLabs");
            assert_eq!(repo.name, "terrier");
        }
    }

    #[test]
    fn rejects_conflicting_hosts_and_malformed_slugs() {
        assert!(Repo::parse("github.com/ScottyLabs/terrier", Some("git.cmu.dev")).is_err());
        assert!(
            Repo::remote("https://github.com/ScottyLabs/terrier", Some("git.cmu.dev")).is_err()
        );
        for slug in [
            "../terrier",
            "ScottyLabs/..",
            "ScottyLabs/",
            "ScottyLabs/a?token",
            "a/b/c/d",
        ] {
            assert!(Repo::parse(slug, None).is_err());
        }
    }
}
