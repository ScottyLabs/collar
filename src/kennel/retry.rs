//! Explicit, single-delivery Kennel retries using Forgejo's push-test API.

use anyhow::{Context, Result, bail, ensure};
use reqwest::Method;
use serde::{Deserialize, Serialize};

use crate::{client::Api, output::Output, pr, repo::Repo, ui};

mod hooks;

#[derive(clap::Args)]
pub struct Args {
    /// Branch to retry; omitted selects the repository's default branch.
    #[arg(conflicts_with = "pr")]
    branch: Option<String>,
    /// PR selector; currently rejected because push tests cannot retry previews.
    #[arg(long)]
    pr: Option<String>,
    /// Select a Kennel webhook when more than one is configured.
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..))]
    hook: Option<u64>,
    #[command(flatten)]
    output: Output,
}

#[derive(Deserialize)]
struct Version {
    version: String,
}

#[derive(Deserialize)]
struct Repository {
    default_branch: String,
}

#[derive(Deserialize)]
struct Branch {
    name: String,
    commit: Commit,
}

#[derive(Deserialize)]
struct Commit {
    id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Report<'a> {
    repository: String,
    branch: &'a str,
    head_ref_oid: &'a str,
    hook_id: u64,
    trigger_accepted: bool,
    head_unchanged: Option<bool>,
    revision_verified: bool,
    delivery_confirmed: bool,
    build_start_confirmed: bool,
}

pub fn run(args: Args, api: &Api, repo: &Repo) -> Result<()> {
    args.output.validate_fields(&[
        "repository",
        "branch",
        "headRefOid",
        "hookId",
        "triggerAccepted",
        "headUnchanged",
        "revisionVerified",
        "deliveryConfirmed",
        "buildStartConfirmed",
    ])?;
    api.require_token()?;
    let branch = target(api, repo, args.branch.as_deref(), args.pr.as_deref())?;
    let sha = head(api, repo, &branch)?;
    let hook_id = hooks::prepare(api, repo, &branch, args.hook)?;
    ensure!(
        head(api, repo, &branch)? == sha,
        "branch head changed while preparing retry; inspect the new revision before retrying"
    );
    let id = hook_id.to_string();
    let mut hooks_path = repo.path().to_vec();
    hooks_path.extend(["hooks", &id, "tests"]);
    api.text(Method::POST, &hooks_path, &[("ref".into(), format!("refs/heads/{branch}"))], None)
        .context("could not confirm retry acceptance; check webhook administration permission; after a timeout or server error inspect Forgejo deliveries and Kennel before another attempt")?;
    let after = head(api, repo, &branch);
    let unchanged = after.as_ref().ok().map(|value| value == &sha);
    let report = Report {
        repository: format!("{}/{}", repo.owner, repo.name),
        branch: &branch,
        head_ref_oid: &sha,
        hook_id,
        trigger_accepted: true,
        head_unchanged: unchanged,
        revision_verified: false,
        delivery_confirmed: false,
        build_start_confirmed: false,
    };
    args.output.print(&report, || {
        ui::heading("Kennel retry trigger accepted");
        ui::detail("Branch", &branch);
        ui::detail("Observed commit", &sha);
        ui::detail("Verification", "Delivery and build start are unconfirmed. Forgejo cannot atomically pin branch and SHA.");
        println!("Inspect Forgejo webhook deliveries and Kennel reports before sending another retry.");
    })?;
    match after {
        Ok(value) if value == sha => Ok(()),
        Ok(_) => bail!("trigger accepted, but the branch head changed; payload revision is uncertain; inspect deliveries before another retry"),
        Err(error) => Err(error)
            .context("trigger accepted, but checking the branch failed; inspect deliveries before another retry"),
    }
}

fn head(api: &Api, repo: &Repo, branch: &str) -> Result<String> {
    let mut path = repo.path().to_vec();
    path.extend(["branches", branch]);
    let value: Branch = api
        .json(Method::GET, &path, &[], None)
        .context("read retry branch; check that it exists and is visible to your credential")?;
    ensure!(
        value.name == branch
            && matches!(value.commit.id.len(), 40 | 64)
            && value.commit.id.bytes().all(|byte| byte.is_ascii_hexdigit())
            && value.commit.id.bytes().any(|byte| byte != b'0'),
        "forge returned an invalid branch head"
    );
    Ok(value.commit.id)
}

fn target(
    api: &Api,
    repo: &Repo,
    requested: Option<&str>,
    selector: Option<&str>,
) -> Result<String> {
    if let Some(selector) = selector {
        let number = pr::select(api, repo, Some(selector))?;
        let pull = pr::open_head(api, repo, number, None)?;
        bail!(
            "PR #{} at {} cannot be retried with a push test: Kennel previews require a pull-request event for pr-{}; use Forgejo's delivery UI to redeliver the matching pull-request event or add a Kennel retry endpoint",
            pull.number,
            pull.head.sha,
            pull.number
        );
    }
    let branch = match requested {
        Some(branch) => branch
            .strip_prefix("refs/heads/")
            .unwrap_or(branch)
            .to_owned(),
        None => {
            api.json::<Repository>(Method::GET, &repo.path(), &[], None)?
                .default_branch
        }
    };
    ensure!(
        matches!(branch.as_str(), "main" | "staging" | "dev"),
        "Kennel ignores push events for branch {branch:?}; only main, staging, and dev support push retries; PR previews need a pull-request retry endpoint"
    );
    let version: Version = api.json(Method::GET, &["version"], &[], None)?;
    ensure!(
        version
            .version
            .split('.')
            .next()
            .and_then(|major| major.parse::<u64>().ok())
            .is_some_and(|major| major >= 16),
        "targeted webhook tests require a verified Forgejo 16+ ref API; no trigger was sent"
    );
    Ok(branch)
}
