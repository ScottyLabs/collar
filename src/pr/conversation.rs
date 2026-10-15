use std::collections::BTreeMap;

use anyhow::{Result, ensure};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{client::Api, repo::Repo, ui};

#[derive(Deserialize, Serialize)]
struct Author {
    login: String,
    #[serde(flatten)]
    metadata: BTreeMap<String, Value>,
}

#[derive(Deserialize, Serialize)]
struct Discussion {
    id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<Author>,
    #[serde(flatten)]
    metadata: BTreeMap<String, Value>,
}

#[derive(Deserialize, Serialize)]
struct InlineComment {
    id: u64,
    path: String,
    body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    diff_hunk: Option<String>,
    #[serde(flatten)]
    metadata: BTreeMap<String, Value>,
}

#[derive(Deserialize, Serialize)]
struct Review {
    id: u64,
    state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<Author>,
    #[serde(skip_serializing_if = "Option::is_none")]
    comments_count: Option<u64>,
    #[serde(default)]
    comments: Vec<InlineComment>,
    #[serde(flatten)]
    metadata: BTreeMap<String, Value>,
}

#[derive(Serialize)]
pub struct Conversation {
    comments: Vec<Discussion>,
    reviews: Vec<Review>,
}

impl Conversation {
    pub fn load(api: &Api, repo: &Repo, number: u64) -> Result<Self> {
        let number = number.to_string();
        let mut path = repo.path().to_vec();
        path.extend(["issues", &number, "comments"]);
        let comments = api.json(Method::GET, &path, &[], None)?;
        let mut path = repo.path().to_vec();
        path.extend(["pulls", &number, "reviews"]);
        let mut reviews: Vec<Review> = api.list(&path, &[], 5001)?;
        ensure!(
            reviews.len() <= 5000,
            "PR has too many reviews; inspect its conversation in the browser"
        );
        for review in &mut reviews {
            if review.comments_count.is_some_and(|count| count > 0) {
                let id = review.id.to_string();
                let mut comment_path = path.clone();
                comment_path.extend([&id, "comments"]);
                review.comments = api.json(Method::GET, &comment_path, &[], None)?;
            }
        }
        Ok(Self { comments, reviews })
    }

    pub fn print(&self) {
        ui::heading("Discussion");
        if self.comments.is_empty() {
            println!("No discussion.");
        }
        for comment in &self.comments {
            ui::detail(
                comment.user.as_ref().map_or("unknown", |user| &user.login),
                "comment",
            );
            if let Some(body) = &comment.body {
                ui::markdown(body);
            }
        }
        ui::heading("Reviews");
        if self.reviews.is_empty() {
            println!("No reviews.");
        }
        for review in &self.reviews {
            ui::detail(
                review.user.as_ref().map_or("unknown", |user| &user.login),
                &review.state,
            );
            if let Some(body) = &review.body {
                ui::markdown(body);
            }
            for comment in &review.comments {
                ui::detail("File", &comment.path);
                if let Some(hunk) = &comment.diff_hunk {
                    ui::markdown(hunk);
                }
                ui::markdown(&comment.body);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rejects_malformed_review_identity_and_comment_count() {
        for value in [
            json!({
                "id": "not-a-number",
                "state": "APPROVED",
            }),
            json!({
                "id": 1,
                "state": "APPROVED",
                "comments_count": "invalid",
            }),
        ] {
            assert!(serde_json::from_value::<Review>(value).is_err());
        }
    }

    #[test]
    fn preserves_provider_metadata_without_untyped_control_flow() {
        let value = json!({
            "id": 1,
            "state": "APPROVED",
            "custom_field": {"enabled":true},
            "comments": [],
        });
        let review: Review = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(review).unwrap(), value);
    }
}
