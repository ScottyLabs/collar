//! Personal forge inbox using server-side filters and bounded API reads.

use anyhow::{Result, ensure};
use reqwest::Method;
use serde::{Deserialize, Serialize};

use crate::{client::Api, output::Output, ui};

#[derive(clap::Args)]
pub struct Args {
    /// Filter repositories by owner or organization.
    #[arg(long)]
    owner: Option<String>,
    /// Maximum recently updated items per category.
    #[arg(long, default_value_t = 10, value_parser = clap::value_parser!(u32).range(1..=50))]
    limit: u32,
    #[command(flatten)]
    output: Output,
}

#[derive(Deserialize)]
struct Issue {
    number: u64,
    title: String,
    html_url: String,
    updated_at: String,
    repository: Repository,
    pull_request: Option<Pull>,
}

#[derive(Deserialize)]
struct Repository {
    full_name: String,
}

#[derive(Deserialize)]
struct Pull {
    draft: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Item {
    repository: String,
    number: u64,
    title: String,
    url: String,
    updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_draft: Option<bool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Inbox {
    review_requests: Vec<Item>,
    authored_pull_requests: Vec<Item>,
    assigned_issues: Vec<Item>,
}

pub fn run(args: Args, api: &Api) -> Result<()> {
    args.output
        .validate_fields(&["reviewRequests", "authoredPullRequests", "assignedIssues"])?;
    api.require_token()?;
    let inbox = Inbox {
        review_requests: search(api, &args, "pulls", "review_requested")?,
        authored_pull_requests: search(api, &args, "pulls", "created")?,
        assigned_issues: search(api, &args, "issues", "assigned")?,
    };
    args.output.print(&inbox, || {
        section("Reviews requested", &inbox.review_requests);
        section("Your open pull requests", &inbox.authored_pull_requests);
        section("Issues assigned to you", &inbox.assigned_issues);
    })
}

fn search(api: &Api, args: &Args, kind: &str, filter: &str) -> Result<Vec<Item>> {
    let mut query = vec![
        ("state".into(), "open".into()),
        ("type".into(), kind.into()),
        (filter.into(), "true".into()),
        ("sort".into(), "recentupdate".into()),
        ("page".into(), "1".into()),
        ("limit".into(), args.limit.to_string()),
    ];
    if let Some(owner) = &args.owner {
        ensure!(!owner.trim().is_empty(), "owner must be nonempty");
        query.push(("owner".into(), owner.clone()));
    }
    let issues: Vec<Issue> = api.json(Method::GET, &["repos", "issues", "search"], &query, None)?;
    issues
        .into_iter()
        .take(args.limit as usize)
        .map(|issue| {
            ensure!(
                issue.pull_request.is_some() == (kind == "pulls"),
                "forge returned the wrong issue type for {kind}"
            );
            Ok(Item {
                repository: issue.repository.full_name,
                number: issue.number,
                title: issue.title,
                url: issue.html_url,
                updated_at: issue.updated_at,
                is_draft: issue.pull_request.map(|pull| pull.draft),
            })
        })
        .collect()
}

fn section(title: &str, items: &[Item]) {
    ui::heading(title);
    if items.is_empty() {
        ui::detail("Open items", "None");
    } else {
        ui::table(
            &["Repository", "Number", "Title"],
            &items
                .iter()
                .map(|item| {
                    vec![
                        item.repository.clone(),
                        format!("#{}", item.number),
                        if item.is_draft == Some(true) {
                            format!("[draft] {}", item.title)
                        } else {
                            item.title.clone()
                        },
                    ]
                })
                .collect::<Vec<_>>(),
        );
    }
    println!();
}
