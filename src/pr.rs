mod conversation;
mod models;
mod query;
mod review;

pub use conversation::Conversation;
pub use models::{Comment, LineComment, Pull, Side, Status, User};
pub use query::{Filters, filtered_list, list, select, statuses};
pub use review::review;

use anyhow::{Result, ensure};
use reqwest::Method;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{client::Api, repo::Repo};

pub fn get(api: &Api, repo: &Repo, number: u64) -> Result<Pull> {
    let mut path = repo.path().to_vec();
    let number = number.to_string();
    path.extend(["pulls", &number]);
    api.json(Method::GET, &path, &[], None)
}

pub fn open_head(api: &Api, repo: &Repo, number: u64, expected: Option<&str>) -> Result<Pull> {
    ensure!(
        expected.is_none_or(|value| !value.trim().is_empty()),
        "--match-head-commit must be nonempty"
    );
    let pull = get(api, repo, number)?;
    ensure!(
        pull.number == number,
        "forge returned a different PR number"
    );
    ensure!(
        pull.state == "open" && !pull.merged,
        "PR must be open and unmerged"
    );
    ensure!(!pull.head.sha.is_empty(), "PR has no head commit");
    if let Some(expected) = expected {
        ensure!(
            pull.head.sha == expected,
            "PR head differs from --match-head-commit; inspect it before retrying"
        );
    }
    Ok(pull)
}

pub fn merge(
    api: &Api,
    repo: &Repo,
    pull: &Pull,
    method: &str,
    delete_branch: bool,
    auto: bool,
) -> Result<()> {
    let number = pull.number.to_string();
    let mut path = repo.path().to_vec();
    path.extend(["pulls", &number, "merge"]);
    api.text(
        Method::POST,
        &path,
        &[],
        Some(&json!({
            "Do":method,"head_commit_id":pull.head.sha,"delete_branch_after_merge":delete_branch,
            "force_merge":false,"merge_when_checks_succeed":auto
        })),
    )?;
    Ok(())
}

pub fn update_branch(api: &Api, repo: &Repo, number: u64, style: &str) -> Result<()> {
    let number = number.to_string();
    let mut path = repo.path().to_vec();
    path.extend(["pulls", &number, "update"]);
    api.text(Method::POST, &path, &[("style".into(), style.into())], None)?;
    Ok(())
}

pub fn create(
    api: &Api,
    repo: &Repo,
    title: &str,
    head: &str,
    base: Option<&str>,
    body: &str,
) -> Result<Pull> {
    ensure!(!title.trim().is_empty(), "title must be nonempty");
    ensure!(!head.trim().is_empty(), "head must be nonempty");
    if let Some(base) = base {
        ensure!(!base.trim().is_empty(), "base must be nonempty");
    }
    let base = match base {
        Some(base) => base.to_owned(),
        None => {
            #[derive(Deserialize)]
            struct Repository {
                default_branch: String,
            }
            api.json::<Repository>(Method::GET, &repo.path(), &[], None)?
                .default_branch
        }
    };
    ensure!(!base.trim().is_empty(), "base must be nonempty");
    let mut path = repo.path().to_vec();
    path.push("pulls");
    api.json(
        Method::POST,
        &path,
        &[],
        Some(&json!({
            "title": title,
            "head": head,
            "base": base,
            "body": body,
        })),
    )
}

pub fn edit(api: &Api, repo: &Repo, number: u64, change: &Value) -> Result<Pull> {
    let number = number.to_string();
    let mut path = repo.path().to_vec();
    path.extend(["pulls", &number]);
    api.json(Method::PATCH, &path, &[], Some(change))
}

pub fn comment(api: &Api, repo: &Repo, number: u64, body: &str) -> Result<Comment> {
    get(api, repo, number)?;
    let number = number.to_string();
    let mut path = repo.path().to_vec();
    path.extend(["issues", &number, "comments"]);
    api.json(Method::POST, &path, &[], Some(&json!({ "body": body })))
}

pub fn diff(api: &Api, repo: &Repo, number: u64) -> Result<String> {
    let mut path = repo.path().to_vec();
    let file = format!("{number}.diff");
    path.extend(["pulls", &file]);
    api.text(Method::GET, &path, &[], None)
}
