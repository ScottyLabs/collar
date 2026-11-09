//! Failure reports from nixbot, which builds each push and posts the `nixbot/*` commit statuses

use anyhow::{Context, Result, anyhow, ensure};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::client::{host_url, public_json};

#[derive(Debug, Deserialize, Serialize)]
pub struct Failures {
    pub status: String,
    pub error: Option<String>,
    pub failures: Vec<Failure>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct Failure {
    pub attr: String,
    pub status: String,
    pub error: Option<String>,
    pub log_tail: Option<String>,
}

/// Maps a status's build page, `<origin>/repos/<forge>/<owner>/<name>/builds/<n>`, to its failures API
fn failures_url(target: &str, tail: u64) -> Result<Url> {
    let page = Url::parse(target).context("nixbot status has an invalid target URL")?;
    ensure!(
        page.username().is_empty() && page.password().is_none(),
        "nixbot status target must not contain credentials"
    );

    let mut url = host_url(&page.origin().ascii_serialization())?;
    let segments: Vec<&str> = page
        .path_segments()
        .map(|segments| segments.filter(|segment| !segment.is_empty()).collect())
        .unwrap_or_default();
    let build = segments.len() >= 6
        && segments[0] == "repos"
        && segments[segments.len() - 2] == "builds"
        && segments[segments.len() - 1]
            .bytes()
            .all(|byte| byte.is_ascii_digit())
        && segments
            .iter()
            .all(|segment| !matches!(*segment, "." | ".."));
    ensure!(build, "nixbot status target is not a build page");

    url.path_segments_mut()
        .map_err(|()| anyhow!("nixbot host cannot contain paths"))?
        .push("api")
        .extend(&segments)
        .push("failures");
    url.query_pairs_mut().append_pair("tail", &tail.to_string());
    Ok(url)
}

/// Fetches the failed evaluation or attributes of the nixbot build a status links to
pub fn failures(target: &str, tail: u64) -> Result<Failures> {
    public_json(failures_url(target, tail)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failures_url_accepts_only_build_pages_on_secure_origins() {
        assert_eq!(
            failures_url(
                "https://nixbot.scottylabs.org/repos/gitea/ScottyLabs/dalmatian/builds/44",
                20
            )
            .unwrap()
            .as_str(),
            "https://nixbot.scottylabs.org/api/repos/gitea/ScottyLabs/dalmatian/builds/44/failures?tail=20"
        );
        for target in [
            "http://nixbot.example/repos/gitea/o/n/builds/1",
            "https://user:secret@nixbot.example/repos/gitea/o/n/builds/1",
            "https://nixbot.example/repos/gitea/o/n/builds/latest",
            "https://nixbot.example/repos/gitea/o/n",
            "https://grafana.example/explore",
        ] {
            assert!(failures_url(target, 20).is_err(), "{target}");
        }
    }
}
