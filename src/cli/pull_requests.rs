use anyhow::{Context, Result, ensure};
use serde_json::{Map, Value, json};

use super::input::{Selection, read_file};
use super::pr_args::PrCommand;
use crate::{checkout, client::Api, pr, repo::Repo, tui, ui};

pub(super) fn run(command: PrCommand, api: &Api, repo: &Repo) -> Result<()> {
    match command {
        PrCommand::Checkout {
            selector,
            branch,
            remote,
        } => checkout::run(
            api,
            repo,
            selector.as_deref(),
            branch.as_deref(),
            remote.as_deref(),
        ),
        PrCommand::Merge {
            selector,
            merge: _,
            squash,
            rebase,
            match_head_commit,
            delete_branch,
            auto,
            output,
        } => {
            let method = if squash {
                "squash"
            } else if rebase {
                "rebase"
            } else {
                "merge"
            };
            output.validate_fields(&[
                "number",
                "headRefOid",
                "method",
                "autoMergeRequested",
                "accepted",
            ])?;
            api.require_token()?;
            let number = pr::select(api, repo, selector.as_deref())?;
            let pull = pr::open_head(api, repo, number, match_head_commit.as_deref())?;
            pr::merge(api, repo, &pull, method, delete_branch, auto)?;
            let value = json!({"number":number,"headRefOid":pull.head.sha,"method":method,"autoMergeRequested":auto,"accepted":true});
            output.print(&value, || {
                println!(
                    "Forgejo accepted the {method} request for PR #{number}; inspected head {}{}",
                    pull.head.sha,
                    if auto {
                        "; automatic merge requested without a pinned head"
                    } else {
                        ""
                    }
                )
            })
        }
        PrCommand::UpdateBranch {
            selector,
            rebase,
            match_head_commit,
            output,
        } => {
            output.validate_fields(&["number", "headRefOid", "method", "accepted"])?;
            api.require_token()?;
            let number = pr::select(api, repo, selector.as_deref())?;
            let pull = pr::open_head(api, repo, number, match_head_commit.as_deref())?;
            let style = if rebase { "rebase" } else { "merge" };
            pr::update_branch(api, repo, number, style)?;
            output.print(&json!({"number":number,"headRefOid":pull.head.sha,"method":style,"accepted":true}), || println!("Forgejo accepted the {style} branch update for PR #{number}; inspected head {}", pull.head.sha))
        }
        PrCommand::Create {
            title,
            head,
            base,
            text,
            output,
        } => {
            output.validate_fields(pr::Pull::FIELDS)?;
            api.require_token()?;
            let body = text.read()?.unwrap_or_default();
            let head = head.map(Ok).unwrap_or_else(crate::repo::branch)?;
            let pull = pr::create(api, repo, &title, &head, base.as_deref(), &body)?;
            output.print(&pull.output(), || println!("{}", pull.html_url))
        }
        PrCommand::Edit {
            selector,
            title,
            base,
            text,
            output,
        } => {
            output.validate_fields(pr::Pull::FIELDS)?;
            api.require_token()?;
            let body = text.read()?;
            let mut change = Map::new();
            for (key, value) in [("title", title), ("base", base), ("body", body)] {
                if let Some(value) = value {
                    if key != "body" {
                        ensure!(!value.trim().is_empty(), "{key} must be nonempty");
                    }
                    change.insert(key.into(), value.into());
                }
            }
            ensure!(
                !change.is_empty(),
                "provide --title, --base, --body, or --body-file"
            );
            let number = pr::select(api, repo, selector.as_deref())?;
            let pull = pr::edit(api, repo, number, &Value::Object(change))?;
            output.print(&pull.output(), || println!("{}", pull.html_url))
        }
        PrCommand::Comment {
            selector,
            text,
            output,
        } => {
            output.validate_fields(&["id", "body", "url", "author"])?;
            api.require_token()?;
            let body = text.read()?.context("provide --body or --body-file")?;
            ensure!(!body.trim().is_empty(), "comment body must be nonempty");
            let number = pr::select(api, repo, selector.as_deref())?;
            let comment = pr::comment(api, repo, number, &body)?;
            output.print(&comment.output(), || println!("{}", comment.html_url))
        }
        PrCommand::Close(selection) => set_pull_state(api, repo, selection, "closed"),
        PrCommand::Reopen(selection) => set_pull_state(api, repo, selection, "open"),
        PrCommand::Browse {
            state,
            limit,
            output,
        } => tui::browse_pull_requests(api, repo, state.as_str(), limit, &output),
        PrCommand::List {
            state,
            limit,
            author,
            base,
            head,
            output,
        } => {
            output.validate_fields(pr::Pull::FIELDS)?;
            let pulls = pr::filtered_list(
                api,
                repo,
                state.as_str(),
                limit,
                &pr::Filters { author, base, head },
            )?;
            let values: Vec<_> = pulls.iter().map(pr::Pull::output).collect();
            output.print(&values, || {
                ui::heading("Pull requests");
                ui::table(
                    &["PR", "State", "Title", "Author"],
                    &pulls
                        .iter()
                        .map(|pull| {
                            vec![
                                format!("#{}", pull.number),
                                if pull.merged {
                                    "merged".into()
                                } else {
                                    pull.state.clone()
                                },
                                pull.title.clone(),
                                pull.user.login.clone(),
                            ]
                        })
                        .collect::<Vec<_>>(),
                );
            })
        }
        PrCommand::View {
            selector,
            comments,
            output,
        } => {
            let mut fields = pr::Pull::FIELDS.to_vec();
            if comments {
                fields.extend(["comments", "reviews"]);
            }
            output.validate_fields(&fields)?;
            let number = pr::select(api, repo, selector.as_deref())?;
            let pull = pr::get(api, repo, number)?;
            let mut value = pull.output();
            let conversation = if comments {
                let conversation = pr::Conversation::load(api, repo, number)?;
                let fields = serde_json::to_value(&conversation)?;
                let object = fields.as_object().context("serialize PR conversation")?;
                value
                    .as_object_mut()
                    .context("serialize pull request")?
                    .extend(object.clone());
                Some(conversation)
            } else {
                None
            };
            output.print(&value, || {
                pull.print();
                if let Some(conversation) = &conversation {
                    conversation.print();
                }
            })
        }
        PrCommand::Diff { selector } => {
            let number = pr::select(api, repo, selector.as_deref())?;
            print!("{}", pr::diff(api, repo, number)?);
            Ok(())
        }
        PrCommand::Checks(selection) => {
            selection
                .output
                .validate_fields(&["name", "state", "description", "link"])?;
            let number = pr::select(api, repo, selection.selector.as_deref())?;
            let pull = pr::get(api, repo, number)?;
            let statuses = pr::statuses(api, repo, &pull.head.sha)?;
            let values: Vec<_> = statuses.iter().map(|status| json!({ "name": status.context, "state": status.state, "description": status.description, "link": status.target_url })).collect();
            selection.output.print(&values, || {
                ui::heading(&format!("Checks for PR #{}", pull.number));
                ui::table(
                    &["State", "Check", "Description", "URL"],
                    &statuses
                        .iter()
                        .map(|status| {
                            vec![
                                status.state.clone(),
                                status.context.clone(),
                                status.description.clone().unwrap_or_default(),
                                status.target_url.clone().unwrap_or_default(),
                            ]
                        })
                        .collect::<Vec<_>>(),
                );
            })?;
            ensure!(
                !statuses.is_empty(),
                "no commit statuses reported; required checks are not verified"
            );
            ensure!(
                statuses.iter().all(|status| status.state == "success"),
                "one or more reported commit statuses are not successful"
            );
            Ok(())
        }
        PrCommand::Review(args) => {
            api.require_token()?;
            let number = pr::select(api, repo, args.selector.as_deref())?;
            let body = match args.body_file {
                Some(path) => read_file(&path)?,
                None => args.body.unwrap_or_default(),
            };
            let comments = args
                .inline_comments
                .as_chunks::<4>()
                .0
                .iter()
                .map(|fields| {
                    let line = fields[1]
                        .parse::<u64>()
                        .context("inline comment LINE must be a positive integer")?;
                    ensure!(line > 0, "inline comment LINE must be positive");
                    let side = match fields[2].as_str() {
                        "old" => pr::Side::Old,
                        "new" => pr::Side::New,
                        _ => anyhow::bail!("inline comment SIDE must be old or new"),
                    };
                    Ok(pr::LineComment {
                        path: fields[0].clone(),
                        line,
                        side,
                        body: fields[3].clone(),
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            let event = if args.approve {
                "APPROVED"
            } else if args.request_changes {
                "REQUEST_CHANGES"
            } else {
                "COMMENT"
            };
            let review = pr::review(api, repo, number, event, &body, &comments)?;
            let id = review.id;
            println!("Submitted {event} review {id} on PR #{number}");
            Ok(())
        }
    }
}

fn set_pull_state(api: &Api, repo: &Repo, selection: Selection, state: &str) -> Result<()> {
    selection.output.validate_fields(pr::Pull::FIELDS)?;
    api.require_token()?;
    let number = pr::select(api, repo, selection.selector.as_deref())?;
    let current = pr::get(api, repo, number)?;
    ensure!(!current.merged, "a merged PR cannot be closed or reopened");
    ensure!(current.state != state, "PR is already {state}");
    let pull = pr::edit(api, repo, number, &json!({"state":state}))?;
    selection
        .output
        .print(&pull.output(), || println!("{}", pull.html_url))
}
