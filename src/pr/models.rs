use crate::ui;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

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

    pub fn print(&self) {
        ui::heading(&format!("PR #{} {}", self.number, self.title));
        let state = if self.merged { "merged" } else { &self.state };
        let color = if self.merged {
            ui::Status::Success
        } else if self.state == "closed" {
            ui::Status::Error
        } else {
            ui::Status::Warning
        };
        ui::status("State", state, color);
        ui::detail("Author", &self.user.login);
        ui::detail(
            "Branch",
            &format!("{} -> {}", self.head.name, self.base.name),
        );
        ui::detail("URL", &self.html_url);
        if let Some(body) = self.body.as_deref().filter(|body| !body.trim().is_empty()) {
            println!("\n{}", body);
        }
    }
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

#[derive(Debug, Deserialize, Serialize)]
pub struct Status {
    pub id: u64,
    pub context: String,
    #[serde(rename(deserialize = "status"))]
    pub state: String,
    pub description: Option<String>,
    pub target_url: Option<String>,
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
