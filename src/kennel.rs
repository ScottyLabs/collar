use anyhow::{Result, bail, ensure};
use clap::Subcommand;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    thread,
    time::{Duration, Instant},
};

use crate::{
    client::Api,
    output::Output,
    pr::{self, Pull, Status},
    repo::Repo,
};

const COMMENT_MARKER: &str = "<!-- kennel-deployment -->";
const BUILD: &str = "kennel/build";
const DEPLOY: &str = "kennel/deploy";

#[derive(Subcommand)]
pub enum Command {
    /// Retry a Kennel branch build through its verified Forgejo webhook.
    #[command(
        after_long_help = "Examples:\n  collar kennel retry\n  collar kennel retry staging\n  collar kennel retry dev --hook 12\n\nOmitting BRANCH selects the repository's default branch. Kennel push retries support main, staging, and dev. PR previews require a pull-request event and cannot be retried by Forgejo's push-test endpoint. Requires repository webhook administration permission. An accepted trigger is not proof of delivery or build start; never repeat an uncertain write automatically."
    )]
    Retry(retry::Args),
    /// Find preview links reported in marked Kennel PR comments.
    Preview {
        selector: Option<String>,
        #[command(flatten)]
        output: Output,
    },
    /// Print Kennel build and deploy report links for a PR head.
    Logs {
        selector: Option<String>,
        #[command(flatten)]
        output: Output,
    },
    /// Wait for Kennel build and deployment statuses on the current PR head.
    Wait {
        selector: Option<String>,
        #[arg(long, default_value_t = 120, value_parser = clap::value_parser!(u64).range(1..=300))]
        timeout: u64,
        #[arg(long, default_value_t = 2, value_parser = clap::value_parser!(u64).range(1..=30))]
        interval: u64,
        #[command(flatten)]
        output: Output,
    },
}

pub fn run(command: Command, api: &Api, repo: &Repo) -> Result<()> {
    match command {
        Command::Retry(args) => retry::run(args, api, repo)?,
        Command::Preview { selector, output } => {
            output.validate_fields(&["number", "headRefOid", "comments", "currentness"])?;
            let pull = selected_pull(api, repo, selector.as_deref())?;
            let comments = comments(api, repo, pull.number)?;
            let reports: Vec<_> = comments
                .into_iter()
                .map(|comment| {
                    let revision = comment_revision(&comment.body, &pull.head.sha);
                    json!({
                        "url": comment.html_url,
                        "revision": revision,
                        "previewUrls": urls(&comment.body),
                    })
                })
                .collect();
            ensure!(
                !reports.is_empty(),
                "no marked Kennel deployment comments found for PR #{}",
                pull.number
            );
            let value = json!({
                "number": pull.number,
                "headRefOid": pull.head.sha,
                "comments": reports,
                "currentness": "Comments are historical reports; only a matching reported commit prefix identifies the PR head.",
            });
            output.print(&value, || {
                println!("PR #{} at {}", pull.number, pull.head.sha);
                for comment in &reports {
                    println!(
                        "{} ({})",
                        comment["url"].as_str().unwrap_or(""),
                        comment["revision"].as_str().unwrap_or("unknown")
                    );
                    if let Some(urls) = comment["previewUrls"].as_array() {
                        for url in urls {
                            println!("  {}", url.as_str().unwrap_or(""));
                        }
                    }
                }
                println!("Comments are historical reports; live health is not verified.");
            })?;
        }
        Command::Logs { selector, output } => {
            output.validate_fields(&["number", "headRefOid", "statuses", "note"])?;
            let pull = selected_pull(api, repo, selector.as_deref())?;
            let statuses = relevant_statuses(api, repo, &pull.head.sha)?;
            ensure!(
                !statuses.is_empty(),
                "no Kennel statuses found for PR #{} at {}",
                pull.number,
                pull.head.sha
            );
            let value = json!({
                "number": pull.number,
                "headRefOid": pull.head.sha,
                "statuses": statuses.iter().map(status_value).collect::<Vec<_>>(),
                "note": "Status target URLs point to Kennel reports, commonly build logs. Historical status records do not establish current deployment health.",
            });
            output.print(&value, || {
                println!("PR #{} at {}", pull.number, pull.head.sha);
                for status in &statuses {
                    println!(
                        "{}\t{}\t{}",
                        status.context,
                        status.state,
                        status.target_url.as_deref().unwrap_or("")
                    );
                }
                println!("Forgejo reports only; live health is not verified.");
            })?;
        }
        Command::Wait {
            selector,
            timeout,
            interval,
            output,
        } => {
            output.validate_fields(&["number", "headRefOid", "build", "deploy", "health"])?;
            wait(api, repo, selector.as_deref(), timeout, interval, &output)?;
        }
    }
    Ok(())
}

fn selected_pull(api: &Api, repo: &Repo, selector: Option<&str>) -> Result<Pull> {
    let number = pr::select(api, repo, selector)?;
    let pull = pr::get(api, repo, number)?;
    ensure!(
        pull.number == number,
        "forge returned a different PR number"
    );
    ensure!(!pull.head.sha.is_empty(), "PR has no head commit");
    Ok(pull)
}

#[derive(Deserialize)]
struct Comment {
    body: String,
    html_url: String,
}

fn comments(api: &Api, repo: &Repo, number: u64) -> Result<Vec<Comment>> {
    let mut path = repo.path().to_vec();
    let number = number.to_string();
    path.extend(["issues", &number, "comments"]);
    Ok(api
        .json::<Vec<Comment>>(reqwest::Method::GET, &path, &[], None)?
        .into_iter()
        .filter(|comment| comment.body.contains(COMMENT_MARKER))
        .collect())
}

fn urls(body: &str) -> Vec<String> {
    body.lines()
        .filter_map(|line| {
            let columns: Vec<_> = line.split('|').map(str::trim).collect();
            columns.iter().find_map(|cell| valid_url(cell))
        })
        .collect()
}

fn valid_url(value: &str) -> Option<String> {
    let url = url::Url::parse(value).ok()?;
    matches!(url.scheme(), "http" | "https").then_some(())?;
    (url.username().is_empty() && url.password().is_none()).then(|| url.to_string())
}

fn comment_revision(body: &str, head: &str) -> &'static str {
    let Some(revision) = body.lines().find_map(|line| {
        line.strip_prefix("Last updated for commit `")
            .and_then(|rest| rest.split('`').next())
    }) else {
        return "unknown";
    };
    if revision.len() < 7
        || revision.len() > head.len()
        || !revision.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return "unknown";
    }
    if head.starts_with(revision) {
        "matches-head-prefix"
    } else {
        "older-or-different"
    }
}

fn relevant_statuses(api: &Api, repo: &Repo, sha: &str) -> Result<Vec<Status>> {
    Ok(pr::statuses(api, repo, sha)?
        .into_iter()
        .filter(|status| matches!(status.context.as_str(), BUILD | DEPLOY))
        .collect())
}

fn status_value(status: &Status) -> Value {
    json!({
        "context": status.context,
        "status": status.state,
        "description": status.description,
        "targetUrl": status.target_url,
    })
}

fn wait(
    api: &Api,
    repo: &Repo,
    selector: Option<&str>,
    timeout: u64,
    interval: u64,
    output: &Output,
) -> Result<()> {
    let number = pr::select(api, repo, selector)?;
    let initial = pr::get(api, repo, number)?;
    ensure!(
        initial.number == number && initial.state == "open" && !initial.merged,
        "PR must be open and unmerged"
    );
    ensure!(!initial.head.sha.is_empty(), "PR has no head commit");
    let sha = initial.head.sha;
    let deadline = Instant::now() + Duration::from_secs(timeout);
    loop {
        let pull = pr::get(api, repo, number)?;
        ensure!(
            pull.state == "open" && !pull.merged,
            "PR #{number} closed while waiting"
        );
        ensure!(
            pull.number == number && pull.head.sha == sha,
            "PR head changed while waiting; rerun for the new revision"
        );
        let statuses = relevant_statuses(api, repo, &sha)?;
        let build = statuses.iter().find(|status| status.context == BUILD);
        let deploy = statuses.iter().find(|status| status.context == DEPLOY);
        if let Some(status) =
            build.filter(|status| status.state == "failure" || status.state == "error")
        {
            bail!(
                "Kennel build failed for {sha}: {}",
                status.description.as_deref().unwrap_or("no description")
            );
        }
        if let Some(status) =
            deploy.filter(|status| status.state == "failure" || status.state == "error")
        {
            bail!(
                "Kennel deployment failed for {sha}: {}",
                status.description.as_deref().unwrap_or("no description")
            );
        }
        if build.is_some_and(|status| status.state == "success")
            && deploy.is_some_and(|status| status.state == "success")
        {
            let current = pr::get(api, repo, number)?;
            ensure!(
                current.number == number && current.state == "open" && !current.merged,
                "PR #{number} closed while waiting"
            );
            ensure!(
                current.head.sha == sha,
                "PR head changed while waiting; rerun for the new revision"
            );
            let value = json!({
                "number": number,
                "headRefOid": sha,
                "build": build.map(status_value),
                "deploy": deploy.map(status_value),
                "health": "Kennel reported deployment success for this commit; live health was not checked.",
            });
            output.print(&value, || println!("Kennel build and deployment succeeded for PR #{number} at {sha}. Live health was not checked."))?;
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(timeout_error(&sha, build, deploy)?);
        }
        thread::sleep(
            Duration::from_secs(interval).min(deadline.saturating_duration_since(Instant::now())),
        );
        if Instant::now() >= deadline {
            return Err(timeout_error(&sha, build, deploy)?);
        }
    }
}

fn timeout_error(
    sha: &str,
    build: Option<&Status>,
    deploy: Option<&Status>,
) -> Result<anyhow::Error> {
    Ok(anyhow::anyhow!(
        "timed out waiting for Kennel build/deploy reports for {sha}; latest reports: {}",
        serde_json::to_string(&json!({
            "build": build.map(status_value),
            "deploy": deploy.map(status_value),
        }))?
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_contract_marks_only_valid_commit_prefixes_and_public_web_urls() {
        let head = "abcdef0123456789abcdef0123456789abcdef01";
        let comment = "<!-- kennel-deployment -->\n| app | https://preview.example/path |\nLast updated for commit `abcdef0`.\n";
        assert_eq!(comment_revision(comment, head), "matches-head-prefix");
        assert_eq!(
            urls(comment),
            ["https://preview.example/path"].map(str::to_owned)
        );
        assert_eq!(
            comment_revision("Last updated for commit ``.", head),
            "unknown"
        );
        assert_eq!(
            comment_revision("Last updated for commit `not-a-sha`.", head),
            "unknown"
        );
        assert_eq!(
            comment_revision("Last updated for commit `deadbee`.", head),
            "older-or-different"
        );
        assert_eq!(
            urls("| app | https://user:pass@example.test |\n| app | javascript:alert(1) |"),
            Vec::<String>::new()
        );
    }
}
mod retry;
