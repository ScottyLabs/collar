use anyhow::{Context, Result, ensure};
use reqwest::Method;
use serde::{Deserialize, Serialize};

use super::{select_read, select_write};
use crate::{cli::pr_args::ReviewersArgs, client::Api, pr, repo::Repo, tui, ui};

const FIELDS: &[&str] = &["number", "reviewers", "teams"];

#[derive(Deserialize)]
struct Requests {
    #[serde(flatten)]
    pull: pr::Pull,
    #[serde(default)]
    requested_reviewers: Vec<pr::User>,
    #[serde(default)]
    requested_reviewers_teams: Vec<Team>,
}

#[derive(Deserialize)]
struct Team {
    name: String,
}

#[derive(Serialize)]
struct Change<'a> {
    reviewers: &'a [String],
}

#[derive(Serialize)]
struct Report {
    number: u64,
    reviewers: Vec<String>,
    teams: Vec<String>,
}

pub(super) fn run(api: &Api, repo: &Repo, mut args: ReviewersArgs) -> Result<()> {
    if args.pick {
        tui::require_terminal()?;
    }
    let changing = args.pick || !args.add.is_empty() || !args.remove.is_empty();
    let users = if args.add.is_empty() {
        &args.remove
    } else {
        &args.add
    };
    for user in users {
        ensure!(
            !user.is_empty()
                && user
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')),
            "reviewer must be a username: {user}"
        );
    }
    let number = if changing {
        select_write(api, repo, args.selector.as_deref(), &args.output, FIELDS)?
    } else {
        select_read(api, repo, args.selector.as_deref(), &args.output, FIELDS)?
    };
    let number_string = number.to_string();
    let mut path = repo.path().to_vec();
    path.extend(["pulls", &number_string]);
    let mut requests: Requests = api.json(Method::GET, &path, &[], None)?;
    ensure!(
        requests.pull.number == number,
        "forge returned a different PR number"
    );
    if changing {
        ensure!(
            requests.pull.state == "open" && !requests.pull.merged,
            "PR must be open and unmerged"
        );
        if args.pick {
            args.add.push(pick(api, repo, &requests)?);
        }
        let users = if args.add.is_empty() {
            &args.remove
        } else {
            &args.add
        };
        let mut endpoint = path.clone();
        endpoint.push("requested_reviewers");
        let method = if args.add.is_empty() {
            Method::DELETE
        } else {
            Method::POST
        };
        api.text(
            method,
            &endpoint,
            &[],
            Some(&serde_json::to_value(Change { reviewers: users })?),
        )?;
        requests = api.json(Method::GET, &path, &[], None).context(
            "review requests updated, but refreshing the list failed; inspect before retrying",
        )?;
        ensure!(
            requests.pull.number == number,
            "forge returned a different PR number after updating reviewers"
        );
    }
    let report = Report {
        number,
        reviewers: requests
            .requested_reviewers
            .into_iter()
            .map(|user| user.login)
            .collect(),
        teams: requests
            .requested_reviewers_teams
            .into_iter()
            .map(|team| team.name)
            .collect(),
    };
    args.output.print(&report, || {
        ui::heading(&format!("Requested reviewers for PR #{number}"));
        ui::detail(
            "Users",
            &if report.reviewers.is_empty() {
                "None".into()
            } else {
                report.reviewers.join(", ")
            },
        );
        ui::detail(
            "Teams",
            &if report.teams.is_empty() {
                "None".into()
            } else {
                report.teams.join(", ")
            },
        );
    })
}

fn pick(api: &Api, repo: &Repo, requests: &Requests) -> Result<String> {
    let mut path = repo.path().to_vec();
    path.push("reviewers");
    let users: Vec<pr::User> = api.json(Method::GET, &path, &[], None)?;
    let mut names: Vec<_> = users
        .into_iter()
        .map(|user| user.login)
        .filter(|login| {
            login != &requests.pull.user.login
                && !requests
                    .requested_reviewers
                    .iter()
                    .any(|user| &user.login == login)
        })
        .collect();
    names.sort();
    names.dedup();
    ensure!(
        !names.is_empty(),
        "no additional eligible reviewers; use --add USERNAME to request a specific user"
    );
    names
        .get(tui::choose(
            "Request a review from (Enter submits)",
            &names,
        )?)
        .cloned()
        .context("reviewer selection is out of range")
}
