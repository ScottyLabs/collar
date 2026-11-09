//! Deploy state from Kennel's public HTTP API

use anyhow::{Context, Result, anyhow};
use kennel_config::api::{Branch, Health};
use url::Url;

use crate::client::{host_url, public_json};

const DEFAULT_ORIGIN: &str = "https://kennel.scottylabs.org";

fn origin() -> Result<Url> {
    let origin = std::env::var("COLLAR_KENNEL").unwrap_or_else(|_| DEFAULT_ORIGIN.into());
    host_url(&origin).context("invalid COLLAR_KENNEL")
}

/// A project branch's deploy request and deployments, where the project is its repository name
pub fn branch(project: &str, branch: &str) -> Result<Branch> {
    let mut url = origin()?;
    url.path_segments_mut()
        .map_err(|()| anyhow!("Kennel origin cannot contain paths"))?
        .extend(["projects", project, "branches", branch]);

    public_json(url).with_context(|| {
        format!("read Kennel's {branch} branch of {project}; Kennel only knows projects that governance enables it for")
    })
}

/// The pull request branch Kennel deploys previews from
pub fn preview_branch(number: u64) -> String {
    format!("pr-{number}")
}

pub fn health_label(health: Option<&Health>) -> String {
    match health {
        None => "-".into(),
        Some(health) if !health.active => health.active_state.clone(),
        Some(health) => match health.app_healthy {
            Some(true) => "healthy".into(),
            Some(false) => "failing its health check".into(),
            None => "running".into(),
        },
    }
}
