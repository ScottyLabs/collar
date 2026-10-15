use super::{LineComment, Side, diff, get};
use crate::{client::Api, repo::Repo};
use anyhow::{Context, Result, ensure};
use reqwest::Method;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
pub struct SubmittedReview {
    pub id: u64,
}

pub fn review(
    api: &Api,
    repo: &Repo,
    number: u64,
    event: &str,
    body: &str,
    comments: &[LineComment],
) -> Result<SubmittedReview> {
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
        .context("verify the review in the forge before retrying if submission was uncertain")
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
