use anyhow::{Result, bail, ensure};
use clap::Subcommand;
use kennel_config::api::{Deployment, RequestStatus};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    thread,
    time::{Duration, Instant},
};

use crate::{
    client::Api,
    kennel_api, nixbot,
    output::Output,
    pr::{self, Pull, Status},
    repo::Repo,
    ui,
};

const EVAL: &str = "nixbot/nix-eval";
const BUILD: &str = "nixbot/nix-build";

#[derive(Subcommand)]
pub enum Command {
    /// Show the preview URLs Kennel serves for a PR
    Preview {
        selector: Option<String>,
        #[command(flatten)]
        output: Output,
    },
    /// Print nixbot's statuses and failure logs and Kennel's deploy outcome for a PR head
    Logs {
        selector: Option<String>,
        /// Log lines to print for each failed check
        #[arg(long, default_value_t = 50, value_parser = clap::value_parser!(u64).range(1..=1000))]
        tail: u64,
        #[command(flatten)]
        output: Output,
    },
    /// Wait for the nixbot build and Kennel deployment on the current PR head
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
        Command::Preview { selector, output } => {
            output.validate_fields(&["number", "headRefOid", "deployments"])?;
            let pull = selected_pull(api, repo, selector.as_deref())?;
            let branch = kennel_api::branch(&repo.name, &kennel_api::preview_branch(pull.number))?;
            ensure!(
                !branch.deployments.is_empty(),
                "Kennel has no deployments for PR #{}",
                pull.number
            );

            let value = json!({
                "number": pull.number,
                "headRefOid": pull.head.sha,
                "deployments": branch.deployments,
            });
            output.print(&value, || {
                println!("PR #{} at {}", pull.number, pull.head.sha);
                print_deployments(&branch.deployments, &pull.head.sha);
            })?;
        }
        Command::Logs {
            selector,
            tail,
            output,
        } => {
            output.validate_fields(&["number", "headRefOid", "statuses", "failures", "request"])?;
            let pull = selected_pull(api, repo, selector.as_deref())?;
            let statuses = nixbot_statuses(api, repo, &pull.head.sha)?;
            let request =
                kennel_api::branch(&repo.name, &kennel_api::preview_branch(pull.number))?.request;
            ensure!(
                !statuses.is_empty() || request.is_some(),
                "no nixbot statuses or Kennel deploys found for PR #{} at {}",
                pull.number,
                pull.head.sha
            );

            let failures = build_failures(&statuses, tail)?;
            let value = json!({
                "number": pull.number,
                "headRefOid": pull.head.sha,
                "statuses": statuses.iter().map(status_value).collect::<Vec<_>>(),
                "failures": failures,
                "request": request,
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
                for build in &failures {
                    if let Some(error) = &build.error {
                        ui::heading("nixbot evaluation");
                        println!("{error}");
                    }
                    for failure in &build.failures {
                        ui::heading(&format!("{} ({})", failure.attr, failure.status));
                        println!(
                            "{}",
                            failure
                                .log_tail
                                .as_deref()
                                .or(failure.error.as_deref())
                                .unwrap_or("nixbot reported no log")
                        );
                    }
                }
                if let Some(request) = &request {
                    ui::detail("Kennel deploy", &request_label(request));
                }
            })?;
        }
        Command::Wait {
            selector,
            timeout,
            interval,
            output,
        } => {
            output.validate_fields(&["number", "headRefOid", "build", "deploy", "deployments"])?;
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

/// The nixbot statuses on a commit
pub(crate) fn nixbot_statuses(api: &Api, repo: &Repo, sha: &str) -> Result<Vec<Status>> {
    Ok(pr::statuses(api, repo, sha)?
        .into_iter()
        .filter(|status| status.context == EVAL || status.context.starts_with(BUILD))
        .collect())
}

fn failed(status: &Status) -> bool {
    matches!(status.state.as_str(), "failure" | "error")
}

/// Fetches the failure report of each nixbot build with a failed status
fn build_failures(statuses: &[Status], tail: u64) -> Result<Vec<nixbot::Failures>> {
    let builds: BTreeSet<&str> = statuses
        .iter()
        .filter(|status| failed(status))
        .filter_map(|status| status.target_url.as_deref())
        .collect();
    builds
        .into_iter()
        .map(|build| nixbot::failures(build, tail))
        .collect()
}

fn status_value(status: &Status) -> Value {
    json!({
        "context": status.context,
        "status": status.state,
        "description": status.description,
        "targetUrl": status.target_url,
    })
}

pub(crate) fn check_name(context: &str) -> &str {
    match context.rsplit_once('#') {
        Some((_, attr)) => attr,
        None => context.strip_prefix("nixbot/").unwrap_or(context),
    }
}

pub(crate) fn request_label(request: &kennel_config::api::Request) -> String {
    let commit = short(&request.commit);
    match (request.status, &request.failure_stage) {
        (RequestStatus::Failed, Some(stage)) => format!("failed at {stage} for {commit}"),
        (status, _) => format!("{} for {commit}", status_name(status)),
    }
}

fn status_name(status: RequestStatus) -> &'static str {
    match status {
        RequestStatus::Pending => "pending",
        RequestStatus::Deployed => "deployed",
        RequestStatus::Failed => "failed",
        RequestStatus::Skipped => "skipped",
    }
}

pub(crate) fn print_deployments(deployments: &[Deployment], head: &str) {
    ui::table(
        &["Name", "URL", "Commit", "Health"],
        &deployments
            .iter()
            .map(|deployment| {
                vec![
                    deployment.name.clone(),
                    deployment
                        .custom_url
                        .clone()
                        .unwrap_or_else(|| deployment.url.clone()),
                    short(&deployment.commit).into(),
                    kennel_api::health_label(deployment.health.as_ref()),
                ]
            })
            .collect::<Vec<_>>(),
    );
    if deployments
        .iter()
        .any(|deployment| deployment.commit != head)
    {
        println!("Some deployments run an older commit than the PR head.");
    }
}

fn short(commit: &str) -> &str {
    &commit[..commit.len().min(7)]
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
    let branch_name = kennel_api::preview_branch(number);
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

        let statuses = nixbot_statuses(api, repo, &sha)?;
        let find = |context: &str| statuses.iter().find(|status| status.context == context);
        let (eval, build) = (find(EVAL), find(BUILD));
        if let Some(status) = eval.filter(|status| failed(status)) {
            bail!(
                "nixbot evaluation failed for {sha}: {}; run `collar kennel logs {number}` for the errors",
                status.description.as_deref().unwrap_or("no description")
            );
        }
        if let Some(status) = build.filter(|status| failed(status)) {
            bail!(
                "nixbot build failed for {sha}: {}; run `collar kennel logs {number}` for the logs",
                status.description.as_deref().unwrap_or("no description")
            );
        }

        let branch = kennel_api::branch(&repo.name, &branch_name)?;
        let request = branch
            .request
            .as_ref()
            .filter(|request| request.commit == sha);
        match request.map(|request| request.status) {
            Some(RequestStatus::Failed) => bail!(
                "Kennel deployment failed for {sha} at {}",
                request
                    .and_then(|request| request.failure_stage.as_deref())
                    .unwrap_or("an unknown stage")
            ),
            Some(RequestStatus::Skipped) => {
                bail!("Kennel doesn't deploy previews for this project")
            }
            Some(RequestStatus::Deployed)
                if build.is_some_and(|status| status.state == "success") =>
            {
                let current = pr::get(api, repo, number)?;
                ensure!(
                    current.state == "open" && !current.merged,
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
                    "deploy": request,
                    "deployments": branch.deployments,
                });
                output.print(&value, || {
                    println!(
                        "nixbot build and Kennel deployment succeeded for PR #{number} at {sha}."
                    );
                    print_deployments(&branch.deployments, &sha);
                })?;
                return Ok(());
            }
            _ => {}
        }

        if Instant::now() >= deadline {
            return Err(timeout_error(&sha, build, request));
        }
        thread::sleep(
            Duration::from_secs(interval).min(deadline.saturating_duration_since(Instant::now())),
        );
        if Instant::now() >= deadline {
            return Err(timeout_error(&sha, build, request));
        }
    }
}

fn timeout_error(
    sha: &str,
    build: Option<&Status>,
    deploy: Option<&kennel_config::api::Request>,
) -> anyhow::Error {
    let build = build.map_or_else(
        || "no status yet".to_owned(),
        |status| match &status.description {
            Some(description) => format!("{} ({description})", status.state),
            None => status.state.clone(),
        },
    );
    let deploy = deploy.map_or_else(|| "not requested".to_owned(), request_label);

    anyhow::anyhow!(
        "timed out waiting for the nixbot build and Kennel deployment of {sha}: nixbot build {build}, Kennel deploy {deploy}"
    )
}
