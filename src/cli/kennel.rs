use anyhow::{Result, ensure};
use clap::Subcommand;
use reqwest::Method;
use serde::Deserialize;
use serde_json::json;

use super::input::Selection;
use crate::{auth::api, config::Config, kennel, pr, ui};

#[derive(Subcommand)]
pub(super) enum KennelCommand {
    #[command(flatten)]
    Workflows(kennel::Command),
    /// Show build/deploy reports for a PR and historical preview comments.
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
            kennel::run(command, &api, &repo)
        }
        KennelCommand::Status(selection) => {
            let repo = config.repo(repository, host)?;
            let api = api(repo.origin.clone(), config)?;
            let number = pr::select(&api, &repo, selection.selector.as_deref())?;
            let pull = pr::get(&api, &repo, number)?;
            let statuses: Vec<_> = pr::statuses(&api, &repo, &pull.head.sha)?
                .into_iter()
                .filter(|status| {
                    matches!(status.context.as_str(), "kennel/build" | "kennel/deploy")
                })
                .collect();
            #[derive(Deserialize, serde::Serialize)]
            struct Comment {
                body: String,
                html_url: String,
            }
            let mut path = repo.path().to_vec();
            let number_string = number.to_string();
            path.extend(["issues", &number_string, "comments"]);
            let comments: Vec<Comment> = api.json(Method::GET, &path, &[], None)?;
            let comments: Vec<_> = comments
                .into_iter()
                .filter(|comment| comment.body.contains("<!-- kennel-deployment -->"))
                .collect();
            ensure!(
                !statuses.is_empty() || !comments.is_empty(),
                "no Kennel reports found for PR #{number}; check governance registration and the webhook"
            );
            let value = json!({
                "number": number,
                "headRefOid": pull.head.sha,
                "statuses": statuses,
                "comments": comments,
                "liveHealthVerified": false,
            });
            selection.output.print(&value, || {
                ui::heading(&format!("Kennel reports for PR #{number}"));
                ui::detail("Commit", &pull.head.sha);
                ui::table(
                    &["Check", "State", "URL"],
                    &statuses
                        .iter()
                        .map(|status| {
                            vec![
                                status.context.clone(),
                                status.state.clone(),
                                status.target_url.clone().unwrap_or_default(),
                            ]
                        })
                        .collect::<Vec<_>>(),
                );
                for comment in &comments {
                    ui::detail("Preview", &comment.html_url);
                    println!("{}\n", comment.body);
                }
                ui::detail(
                    "Health",
                    "Forgejo reports only; live health is not verified.",
                );
            })
        }
    }
}
