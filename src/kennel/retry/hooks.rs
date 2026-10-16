//! Discover and recheck the canonical Kennel webhook before a write.

use anyhow::{Context, Result, ensure};
use reqwest::Method;
use serde::Deserialize;
use url::Url;

use crate::{client::Api, repo::Repo};

#[derive(Deserialize)]
struct Destination {
    url: Option<String>,
    content_type: Option<String>,
}

#[derive(Deserialize)]
struct Hook {
    id: u64,
    #[serde(rename = "type")]
    kind: String,
    active: bool,
    events: Vec<String>,
    #[serde(default)]
    config: Option<Destination>,
    #[serde(default)]
    metadata: Option<Destination>,
    url: Option<String>,
    content_type: Option<String>,
    #[serde(default)]
    branch_filter: String,
}

impl Hook {
    fn destination(&self) -> Result<Url> {
        let nested = self
            .metadata
            .as_ref()
            .and_then(|value| value.url.as_deref())
            .or_else(|| self.config.as_ref().and_then(|value| value.url.as_deref()));
        let url = Url::parse(
            nested
                .or(self.url.as_deref())
                .context("webhook has no target URL")?,
        )
        .context("webhook target URL is invalid")?;
        ensure!(
            url.scheme() == "https"
                && url.host_str() == Some("kennel.scottylabs.org")
                && url.port_or_known_default() == Some(443)
                && url.path() == "/webhook"
                && url.username().is_empty()
                && url.password().is_none()
                && url.fragment().is_none()
                && url.query_pairs().all(|(key, _)| key == "namespace"),
            "webhook does not target the canonical Kennel endpoint"
        );
        for candidate in [
            self.url.as_deref(),
            self.config.as_ref().and_then(|value| value.url.as_deref()),
            self.metadata
                .as_ref()
                .and_then(|value| value.url.as_deref()),
        ]
        .into_iter()
        .flatten()
        {
            ensure!(
                Url::parse(candidate).ok().as_ref() == Some(&url),
                "webhook target metadata is inconsistent"
            );
        }
        Ok(url)
    }

    fn validate(&self, branch: &str) -> Result<()> {
        self.destination()?;
        ensure!(
            self.kind == "forgejo"
                && self.active
                && self.events.iter().any(|event| event == "push"),
            "Kennel webhook must be active, use the forgejo payload, and subscribe to push events"
        );
        let content_type = self
            .content_type
            .as_deref()
            .or_else(|| {
                self.metadata
                    .as_ref()
                    .and_then(|value| value.content_type.as_deref())
            })
            .or_else(|| {
                self.config
                    .as_ref()
                    .and_then(|value| value.content_type.as_deref())
            });
        ensure!(
            content_type == Some("json"),
            "Kennel webhook must use JSON payloads"
        );
        ensure!(
            self.branch_filter.is_empty()
                || self.branch_filter == "*"
                || self.branch_filter == branch
                || self.branch_filter == format!("refs/heads/{branch}"),
            "cannot verify this webhook's branch filter for {branch}; inspect its governance configuration"
        );
        Ok(())
    }
}

pub(super) fn prepare(api: &Api, repo: &Repo, branch: &str, id: Option<u64>) -> Result<u64> {
    let mut hooks_path = repo.path().to_vec();
    hooks_path.push("hooks");
    let hooks: Vec<Hook> = api.list(&hooks_path, &[], 501).context(
        "discover Kennel webhook; repository webhook administrator permission is required",
    )?;
    ensure!(
        hooks.len() <= 500,
        "too many repository hooks to safely discover Kennel"
    );
    let selected = select_hook(&hooks, id)?;
    selected.validate(branch)?;
    let id = selected.id.to_string();
    hooks_path.push(&id);
    let current: Hook = api
        .json(Method::GET, &hooks_path, &[], None)
        .context("recheck selected Kennel webhook; no trigger was sent")?;
    current.validate(branch)?;
    ensure!(
        current.id == selected.id && current.destination()? == selected.destination()?,
        "Kennel webhook changed while preparing retry; no trigger was sent"
    );
    Ok(selected.id)
}

fn select_hook(hooks: &[Hook], id: Option<u64>) -> Result<&Hook> {
    if let Some(id) = id {
        let hook = hooks
            .iter()
            .find(|hook| hook.id == id)
            .context("selected webhook ID is not in this repository")?;
        hook.destination()?;
        return Ok(hook);
    }
    let candidates: Vec<_> = hooks
        .iter()
        .filter(|hook| hook.destination().is_ok())
        .collect();
    ensure!(
        !candidates.is_empty(),
        "no canonical Kennel webhook found; enable the repository's Kennel feature in governance or select its hook with --hook ID"
    );
    ensure!(
        candidates.len() == 1,
        "multiple Kennel webhooks found (IDs: {}); select one with --hook ID",
        candidates
            .iter()
            .map(|hook| hook.id.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );
    candidates
        .first()
        .copied()
        .context("missing Kennel webhook")
}
