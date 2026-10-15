//! Borrowed, opt-in conversation views without opaque provider metadata.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use super::Conversation;

impl Conversation {
    pub fn output(&self, compact: bool) -> ConversationView<'_> {
        ConversationView {
            inner: if compact {
                View::Compact(CompactConversation {
                    comments: self
                        .comments
                        .iter()
                        .map(|comment| CompactComment {
                            id: comment.id,
                            body: comment.body.as_deref(),
                            user: comment
                                .user
                                .as_ref()
                                .map(|user| CompactAuthor { login: &user.login }),
                            resolver: None,
                            path: None,
                            diff_hunk: None,
                            metadata: compact_metadata(&comment.metadata),
                        })
                        .collect(),
                    reviews: self
                        .reviews
                        .iter()
                        .map(|review| CompactReview {
                            id: review.id,
                            state: &review.state,
                            body: review.body.as_deref(),
                            user: review
                                .user
                                .as_ref()
                                .map(|user| CompactAuthor { login: &user.login }),
                            comments_count: review.comments_count,
                            comments: review
                                .comments
                                .iter()
                                .map(|comment| CompactComment {
                                    id: comment.id,
                                    body: Some(&comment.body),
                                    user: comment
                                        .metadata
                                        .get("user")
                                        .and_then(|user| user.get("login"))
                                        .and_then(Value::as_str)
                                        .map(|login| CompactAuthor { login }),
                                    resolver: comment
                                        .metadata
                                        .get("resolver")
                                        .and_then(|user| user.get("login"))
                                        .and_then(Value::as_str)
                                        .map(|login| CompactAuthor { login }),
                                    path: Some(&comment.path),
                                    diff_hunk: comment.diff_hunk.as_deref(),
                                    metadata: compact_metadata(&comment.metadata),
                                })
                                .collect(),
                            metadata: compact_metadata(&review.metadata),
                        })
                        .collect(),
                })
            } else {
                View::Full(self)
            },
        }
    }
}

#[derive(Serialize)]
pub struct ConversationView<'a> {
    #[serde(flatten)]
    inner: View<'a>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum View<'a> {
    Full(&'a Conversation),
    Compact(CompactConversation<'a>),
}

#[derive(Serialize)]
struct CompactConversation<'a> {
    comments: Vec<CompactComment<'a>>,
    reviews: Vec<CompactReview<'a>>,
}

#[derive(Serialize)]
struct CompactAuthor<'a> {
    login: &'a str,
}

#[derive(Serialize)]
struct CompactComment<'a> {
    id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<CompactAuthor<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resolver: Option<CompactAuthor<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    diff_hunk: Option<&'a str>,
    #[serde(flatten)]
    metadata: BTreeMap<&'a str, &'a Value>,
}

#[derive(Serialize)]
struct CompactReview<'a> {
    id: u64,
    state: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<CompactAuthor<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    comments_count: Option<u64>,
    comments: Vec<CompactComment<'a>>,
    #[serde(flatten)]
    metadata: BTreeMap<&'a str, &'a Value>,
}

fn compact_metadata(metadata: &BTreeMap<String, Value>) -> BTreeMap<&str, &Value> {
    [
        "state",
        "url",
        "html_url",
        "pull_request_url",
        "created_at",
        "updated_at",
        "submitted_at",
        "resolved",
        "resolved_at",
        "in_reply_to_id",
        "pull_request_review_id",
        "position",
        "original_position",
        "line",
        "original_line",
        "start_line",
        "original_start_line",
        "side",
        "start_side",
        "old_position",
        "new_position",
        "old_line_num",
        "new_line_num",
        "commit_id",
        "original_commit_id",
    ]
    .into_iter()
    .filter_map(|field| metadata.get(field).map(|value| (field, value)))
    .collect()
}
