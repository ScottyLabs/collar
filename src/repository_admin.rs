use anyhow::{Result, ensure};
use clap::{Subcommand, ValueEnum};
use reqwest::Method;
use serde_json::{Value, json};

use crate::{client::Api, repo::Repo, ui};

#[derive(Clone, Copy, ValueEnum)]
pub enum OwnerType {
    Org,
    User,
}

#[derive(Subcommand)]
pub enum Command {
    /// Create a repository for an organization or your account.
    Create {
        name: String,
        #[arg(long)]
        owner: String,
        #[arg(long, value_enum)]
        owner_type: OwnerType,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        private: bool,
    },
    /// Fork the selected repository, optionally into an organization.
    Fork {
        #[arg(long)]
        organization: Option<String>,
        #[arg(long)]
        name: Option<String>,
    },
    /// Change selected repository metadata.
    Edit {
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        homepage: Option<String>,
        #[arg(long)]
        default_branch: Option<String>,
        #[arg(long, conflicts_with = "public")]
        private: bool,
        #[arg(long, conflicts_with = "private")]
        public: bool,
    },
    /// Archive the selected repository.
    Archive,
    /// Unarchive the selected repository.
    Unarchive,
    /// Delete the selected repository permanently.
    Delete {
        /// Confirm deletion by entering OWNER/REPO exactly.
        #[arg(long, value_name = "OWNER/REPO", required = true)]
        confirm: String,
    },
    /// Synchronize a selected Forgejo mirror.
    MirrorSync,
}

pub fn run(command: Command, api: &Api, repo: &Repo) -> Result<()> {
    match command {
        Command::Create {
            name,
            owner,
            owner_type,
            description,
            private,
        } => {
            ensure!(valid_part(&name), "invalid repository name");
            ensure!(valid_part(&owner), "invalid repository owner");
            if matches!(owner_type, OwnerType::User) {
                api.require_token()?;
                #[derive(serde::Deserialize)]
                struct User {
                    login: String,
                }
                let user: User = api.json(Method::GET, &["user"], &[], None)?;
                ensure!(
                    user.login.eq_ignore_ascii_case(&owner),
                    "--owner must match the authenticated account ({})",
                    user.login
                );
            }
            let mut body = serde_json::Map::new();
            body.insert("name".into(), name.into());
            body.insert("private".into(), private.into());
            if let Some(description) = description {
                body.insert("description".into(), description.into());
            }
            let path = match owner_type {
                OwnerType::Org => vec!["orgs", owner.as_str(), "repos"],
                OwnerType::User => vec!["user", "repos"],
            };
            let result: Value = api.json(Method::POST, &path, &[], Some(&Value::Object(body)))?;
            print_repository(&result, "Created")
        }
        Command::Fork { organization, name } => {
            if let Some(org) = &organization {
                ensure!(valid_part(org), "invalid organization name");
            }
            if let Some(name) = &name {
                ensure!(valid_part(name), "invalid repository name");
            }
            let mut body = serde_json::Map::new();
            if let Some(organization) = organization {
                body.insert("organization".into(), organization.into());
            }
            if let Some(name) = name {
                body.insert("name".into(), name.into());
            }
            let result: Value = api.json(
                Method::POST,
                &["repos", &repo.owner, &repo.name, "forks"],
                &[],
                Some(&Value::Object(body)),
            )?;
            print_repository(&result, "Forked")
        }
        Command::Edit {
            description,
            homepage,
            default_branch,
            private,
            public,
        } => {
            ensure!(
                description.is_some()
                    || homepage.is_some()
                    || default_branch.is_some()
                    || private
                    || public,
                "edit requires at least one field"
            );
            let mut body = serde_json::Map::new();
            if let Some(value) = description {
                body.insert("description".into(), value.into());
            }
            if let Some(value) = homepage {
                body.insert("website".into(), value.into());
            }
            if let Some(value) = default_branch {
                body.insert("default_branch".into(), value.into());
            }
            if private {
                body.insert("private".into(), true.into());
            }
            if public {
                body.insert("private".into(), false.into());
            }
            let result: Value =
                api.json(Method::PATCH, &repo.path(), &[], Some(&Value::Object(body)))?;
            print_repository(&result, "Updated")
        }
        Command::Archive => {
            let body = json!({"archived": true});
            let result: Value = api.json(Method::PATCH, &repo.path(), &[], Some(&body))?;
            print_repository(&result, "Archived")
        }
        Command::Unarchive => {
            let body = json!({"archived": false});
            let result: Value = api.json(Method::PATCH, &repo.path(), &[], Some(&body))?;
            print_repository(&result, "Unarchived")
        }
        Command::Delete { confirm } => {
            ensure!(
                confirm == format!("{}/{}", repo.owner, repo.name),
                "deletion confirmation must exactly match {}/{}",
                repo.owner,
                repo.name
            );
            api.text(Method::DELETE, &repo.path(), &[], None)?;
            ui::status(
                "Repository",
                &format!("Deleted {}/{}", repo.owner, repo.name),
                ui::Status::Success,
            );
            Ok(())
        }
        Command::MirrorSync => {
            let current: Value = api.json(Method::GET, &repo.path(), &[], None)?;
            ensure!(
                current.get("mirror").and_then(Value::as_bool) == Some(true),
                "selected repository is not a mirror"
            );
            api.text(
                Method::POST,
                &["repos", &repo.owner, &repo.name, "mirror-sync"],
                &[],
                None,
            )?;
            ui::status(
                "Mirror",
                &format!("Synchronized {}/{}", repo.owner, repo.name),
                ui::Status::Success,
            );
            Ok(())
        }
    }
}

fn valid_part(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

fn print_repository(value: &Value, action: &str) -> Result<()> {
    let full_name = value
        .get("full_name")
        .and_then(Value::as_str)
        .filter(|name| !name.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("Forgejo response omitted repository full_name; the operation may have succeeded, inspect the repository before retrying"))?;
    let url = value
        .get("html_url")
        .and_then(Value::as_str)
        .filter(|url| !url.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("Forgejo response omitted repository html_url; the operation may have succeeded, inspect the repository before retrying"))?;
    ui::heading(&format!("{action} repository {full_name}"));
    ui::detail("URL", url);
    if let Some(description) = value
        .get("description")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
    {
        ui::detail("Description", description);
    }
    if let Some(private) = value.get("private").and_then(Value::as_bool) {
        ui::detail("Visibility", if private { "private" } else { "public" });
    }
    if let Some(archived) = value.get("archived").and_then(Value::as_bool) {
        ui::detail("Archived", if archived { "yes" } else { "no" });
    }
    Ok(())
}
