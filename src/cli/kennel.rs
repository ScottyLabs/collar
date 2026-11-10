use anyhow::{Result, ensure};
use clap::Subcommand;
use serde_json::json;

use super::input::Selection;
use crate::{auth::api, config::Config, kennel, kennel_api, pr, ui};

#[derive(Subcommand)]
pub(super) enum KennelCommand {
    #[command(flatten)]
    Workflows(kennel::Command),
    /// Show nixbot's checks and Kennel's deploy and deployments for a PR
    Status(Selection),
}

pub(super) fn run(
    command: KennelCommand,
    config: &Config,
    repository: Option<&str>,
    host: Option<&str>,
) -> Result<()> {
    match command {
        KennelCommand::Workflows(command) => {
            let repo = config.repo(repository, host)?;
            let api = api(repo.origin.clone(), config)?;
            kennel::run(command, &api, &repo, config)
        }
        KennelCommand::Status(selection) => {
            selection.output.validate_fields(&[
                "number",
                "headRefOid",
                "statuses",
                "request",
                "deployments",
            ])?;
            let repo = config.repo(repository, host)?;
            let api = api(repo.origin.clone(), config)?;
            let number = pr::select(&api, &repo, selection.selector.as_deref())?;
            let pull = pr::get(&api, &repo, number)?;
            let statuses = kennel::nixbot_statuses(&api, &repo, &pull.head.sha)?;
            let branch = kennel_api::branch(&repo.name, &kennel_api::preview_branch(number))?;
            ensure!(
                !statuses.is_empty() || branch.request.is_some(),
                "no nixbot statuses or Kennel deploys found for PR #{number}"
            );

            let value = json!({
                "number": number,
                "headRefOid": pull.head.sha,
                "statuses": statuses,
                "request": branch.request,
                "deployments": branch.deployments,
            });
            selection.output.print(&value, || {
                ui::heading(&format!("PR #{number}"));
                ui::detail("Commit", &pull.head.sha);
                ui::table(
                    &["Check", "State", "URL"],
                    &statuses
                        .iter()
                        .map(|status| {
                            vec![
                                kennel::check_name(&status.context).to_owned(),
                                status.state.clone(),
                                status.target_url.clone().unwrap_or_default(),
                            ]
                        })
                        .collect::<Vec<_>>(),
                );
                if let Some(request) = &branch.request {
                    ui::detail("Kennel deploy", &kennel::request_label(request));
                }
                if !branch.deployments.is_empty() {
                    kennel::print_deployments(&branch.deployments, &pull.head.sha);
                }
            })
        }
    }
}
