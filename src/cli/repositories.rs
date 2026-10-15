use std::path::PathBuf;

use anyhow::Result;
use clap::Subcommand;
use reqwest::Method;
use serde::Deserialize;
use serde_json::json;

use crate::{auth::api, config::Config, output::Output};

#[derive(Subcommand)]
pub(super) enum RepoCommand {
    /// Fork the selected repository without changing its settings.
    Fork {
        /// Fork into this organization instead of your account.
        #[arg(long)]
        organization: Option<String>,
        /// Name of the fork; defaults to the source repository name.
        #[arg(long)]
        name: Option<String>,
    },
    /// Clone using the repository's SSH URL and your existing Git authentication.
    Clone {
        repository: String,
        directory: Option<PathBuf>,
        /// Use HTTPS instead of the default SSH transport.
        #[arg(long)]
        https: bool,
    },
    /// View a repository.
    View {
        /// Repository as [HOST/]OWNER/REPO.
        repository: Option<String>,
        #[command(flatten)]
        output: Output,
    },
}

pub(super) fn run(
    command: RepoCommand,
    config: &Config,
    selection: Option<&str>,
    host: Option<&str>,
) -> Result<()> {
    match command {
        RepoCommand::Clone {
            repository,
            directory,
            https,
        } => {
            let repo = config.repo(Some(&repository), host)?;
            let api = api(repo.origin.clone(), config)?;
            crate::repo::clone(&api, &repo, directory.as_deref(), https)
        }
        RepoCommand::Fork { organization, name } => {
            let repo = config.repo(selection, host)?;
            let api = api(repo.origin.clone(), config)?;
            crate::repo::fork(&api, &repo, organization, name)
        }
        RepoCommand::View { repository, output } => {
            let repo = config.repo(repository.as_deref().or(selection), host)?;
            let api = api(repo.origin.clone(), config)?;
            #[derive(Deserialize)]
            struct Repository {
                full_name: String,
                description: Option<String>,
                html_url: String,
                default_branch: String,
                private: bool,
                archived: bool,
            }
            let repository: Repository = api.json(Method::GET, &repo.path(), &[], None)?;
            let value = json!({
                "nameWithOwner": repository.full_name,
                "description": repository.description,
                "url": repository.html_url,
                "defaultBranchRef": { "name": repository.default_branch },
                "isPrivate": repository.private,
                "isArchived": repository.archived,
            });
            output.print(&value, || {
                println!(
                    "{}\n{}\n{}",
                    repository.full_name,
                    repository.description.as_deref().unwrap_or(""),
                    repository.html_url
                )
            })
        }
    }
}
