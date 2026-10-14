use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use url::Url;

use crate::{client::Api, repo::Repo};

#[derive(Deserialize)]
pub struct User {
    pub login: String,
}

#[derive(Deserialize)]
pub struct Branch {
    #[serde(rename = "ref")]
    pub name: String,
    pub sha: String,
    pub label: Option<String>,
}

#[derive(Deserialize)]
pub struct Pull {
    pub number: u64,
    pub title: String,
    pub body: Option<String>,
    pub state: String,
    pub merged: bool,
    pub draft: bool,
    pub html_url: String,
    pub user: User,
    pub head: Branch,
    pub base: Branch,
}

impl Pull {
    pub const FIELDS: &[&str] = &[
        "number",
        "title",
        "body",
        "state",
        "url",
        "author",
        "isDraft",
        "headRefName",
        "headRefOid",
        "baseRefName",
    ];

    pub fn output(&self) -> Value {
        json!({
            "number": self.number,
            "title": self.title,
            "body": self.body.as_deref().unwrap_or(""),
            "state": if self.merged { "MERGED".into() } else { self.state.to_uppercase() },
            "url": self.html_url,
            "author": { "login": self.user.login },
            "isDraft": self.draft,
            "headRefName": self.head.name,
            "headRefOid": self.head.sha,
            "baseRefName": self.base.name,
        })
    }
}

pub fn get(api: &Api, repo: &Repo, number: u64) -> Result<Pull> {
    let mut path = repo.path().to_vec();
    let number = number.to_string();
    path.extend(["pulls", &number]);
    api.json(Method::GET, &path, &[], None)
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
        Some(&json!({"title":title,"head":head,"base":base,"body":body})),
    )
}

pub fn edit(api: &Api, repo: &Repo, number: u64, change: &Value) -> Result<Pull> {
    let number = number.to_string();
    let mut path = repo.path().to_vec();
    path.extend(["pulls", &number]);
    api.json(Method::PATCH, &path, &[], Some(change))
}

#[derive(Deserialize)]
pub struct Comment {
    id: u64,
    body: String,
    pub html_url: String,
    user: User,
}

impl Comment {
    pub fn output(&self) -> Value {
        json!({"id":self.id,"body":self.body,"url":self.html_url,"author":{"login":self.user.login}})
    }
}

pub fn comment(api: &Api, repo: &Repo, number: u64, body: &str) -> Result<Comment> {
    get(api, repo, number)?;
    let number = number.to_string();
    let mut path = repo.path().to_vec();
    path.extend(["issues", &number, "comments"]);
    api.json(Method::POST, &path, &[], Some(&json!({"body":body})))
}

pub fn list(api: &Api, repo: &Repo, state: &str, limit: usize) -> Result<Vec<Pull>> {
    let mut path = repo.path().to_vec();
    path.push("pulls");
    api.list(&path, &[("state".into(), state.into())], limit)
}

pub fn select(api: &Api, repo: &Repo, selector: Option<&str>) -> Result<u64> {
    if let Some(selector) = selector {
        if !selector.is_empty() && selector.bytes().all(|byte| byte.is_ascii_digit()) {
            let number: u64 = selector.parse().context("PR number is too large")?;
            ensure!(number > 0, "PR number must be positive");
            return Ok(number);
        }
        if selector.contains("://") {
            let url = Url::parse(selector).context("invalid PR URL")?;
            ensure!(
                url.origin() == repo.origin.origin()
                    && url.username().is_empty()
                    && url.password().is_none()
                    && url.query().is_none()
                    && url.fragment().is_none(),
                "PR URL must belong to the selected forge and contain no credentials or query"
            );
            let parts: Vec<_> = url.path().trim_matches('/').split('/').collect();
            ensure!(
                parts.len() == 4
                    && parts[0].eq_ignore_ascii_case(&repo.owner)
                    && parts[1].eq_ignore_ascii_case(&repo.name)
                    && matches!(parts[2], "pulls" | "pull"),
                "PR URL must belong to the selected repository"
            );
            let number: u64 = parts[3].parse().context("PR URL must contain a number")?;
            ensure!(number > 0, "PR number must be positive");
            return Ok(number);
        }
    }
    let branch = selector
        .map(str::to_owned)
        .map_or_else(crate::repo::branch, Ok)?;
    let pulls = list(api, repo, "open", usize::MAX)?;
    let matches: Vec<_> = pulls
        .iter()
        .filter(|pull| pull.head.name == branch || pull.head.label.as_deref() == Some(&branch))
        .collect();
    ensure!(
        matches.len() == 1,
        "expected one open PR for branch {branch}, found {}; specify a PR number",
        matches.len()
    );
    Ok(matches[0].number)
}

pub fn diff(api: &Api, repo: &Repo, number: u64) -> Result<String> {
    let mut path = repo.path().to_vec();
    let file = format!("{number}.diff");
    path.extend(["pulls", &file]);
    api.text(Method::GET, &path, &[], None)
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Status {
    pub id: u64,
    pub context: String,
    #[serde(rename(deserialize = "status"))]
    pub state: String,
    pub description: Option<String>,
    pub target_url: Option<String>,
}

pub fn statuses(api: &Api, repo: &Repo, sha: &str) -> Result<Vec<Status>> {
    let mut path = repo.path().to_vec();
    path.extend(["statuses", sha]);
    let statuses: Vec<Status> = api.list(&path, &[], usize::MAX)?;
    let mut latest = BTreeMap::<String, Status>::new();
    for status in statuses {
        if latest
            .get(&status.context)
            .is_none_or(|old| old.id < status.id)
        {
            latest.insert(status.context.clone(), status);
        }
    }
    Ok(latest.into_values().collect())
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Old,
    New,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct LineComment {
    pub path: String,
    pub line: u64,
    pub side: Side,
    pub body: String,
}

pub fn review(
    api: &Api,
    repo: &Repo,
    number: u64,
    event: &str,
    body: &str,
    comments: &[LineComment],
) -> Result<Value> {
    api.require_token()?;
    ensure!(
        event == "APPROVED" || !body.trim().is_empty() || !comments.is_empty(),
        "comments and requested changes require a review body or inline comments"
    );
    let pull = get(api, repo, number)?;
    ensure!(
        pull.state == "open" && !pull.merged,
        "PR is closed; review an open PR"
    );
    ensure!(!pull.head.sha.is_empty(), "PR response has no head commit");
    if !comments.is_empty() {
        validate_comments(&diff(api, repo, number)?, comments)?;
    }
    let current = get(api, repo, number)?;
    ensure!(
        current.head.sha == pull.head.sha,
        "PR head changed while preparing the review; inspect it again"
    );
    ensure!(
        current.state == "open" && !current.merged,
        "PR closed while preparing the review; inspect it again"
    );
    let comments: Vec<_> = comments
        .iter()
        .map(|comment| {
            json!({
                "path": comment.path,
                "body": comment.body,
                "new_position": if matches!(comment.side, Side::New) { comment.line } else { 0 },
                "old_position": if matches!(comment.side, Side::Old) { comment.line } else { 0 },
            })
        })
        .collect();
    let payload =
        json!({ "event": event, "body": body, "commit_id": pull.head.sha, "comments": comments });
    let mut path = repo.path().to_vec();
    let number = number.to_string();
    path.extend(["pulls", &number, "reviews"]);
    api.json(Method::POST, &path, &[], Some(&payload))
}

fn validate_comments(diff: &str, comments: &[LineComment]) -> Result<()> {
    let anchors = crate::diff::anchors(diff)?;
    for comment in comments {
        ensure!(
            !comment.body.trim().is_empty(),
            "inline comment body must be nonempty"
        );
        ensure!(comment.line > 0, "inline comment line must be positive");
        ensure!(
            anchors.contains(&(
                comment.path.clone(),
                matches!(comment.side, Side::New),
                comment.line
            )),
            "{}:{} is not a commentable line on the selected side",
            comment.path,
            comment.line
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_added_removed_and_context_lines() {
        let diff = "diff --git a/src/main.rs b/src/main.rs\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -2,2 +2,2 @@\n same\n-old\n+new\n";
        for (side, line) in [(Side::New, 3), (Side::Old, 3), (Side::New, 2)] {
            validate_comments(
                diff,
                &[LineComment {
                    path: "src/main.rs".into(),
                    line,
                    side,
                    body: "Explain this".into(),
                }],
            )
            .unwrap();
        }
        assert!(
            validate_comments(
                diff,
                &[LineComment {
                    path: "src/main.rs".into(),
                    line: 4,
                    side: Side::New,
                    body: "Outside diff".into()
                }]
            )
            .is_err()
        );
    }

    #[test]
    fn added_content_cannot_replace_the_diff_file_header() {
        let diff = "diff --git a/file.md b/file.md\n--- a/file.md\n+++ b/file.md\n@@ -1 +1,2 @@\n-old\n+++ heading\n+following\n";
        for line in [1, 2] {
            validate_comments(
                diff,
                &[LineComment {
                    path: "file.md".into(),
                    line,
                    side: Side::New,
                    body: "Explain this".into(),
                }],
            )
            .unwrap();
        }
    }
}
