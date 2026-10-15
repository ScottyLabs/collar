use anyhow::{Result, ensure};
use serde::Serialize;

use super::super::input::Selection;
use super::super::pr_args::{BrowseArgs, DiffArgs, ListArgs, ViewArgs};
use super::select_read;
use crate::{client::Api, pr, repo::Repo, tui, ui};

pub(super) fn list(api: &Api, repo: &Repo, args: ListArgs) -> Result<()> {
    let ListArgs {
        state,
        limit,
        author,
        base,
        head,
        output,
    } = args;
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

pub(super) fn view(api: &Api, repo: &Repo, args: ViewArgs) -> Result<()> {
    let ViewArgs {
        selector,
        comments,
        output,
    } = args;
    let mut fields = pr::Pull::FIELDS.to_vec();
    if comments {
        fields.extend(["comments", "reviews"]);
    }
    let number = select_read(api, repo, selector.as_deref(), &output, &fields)?;
    let pull = pr::get(api, repo, number)?;
    let conversation = comments
        .then(|| pr::Conversation::load(api, repo, number))
        .transpose()?;
    let value = PullView {
        pull: pull.output(),
        conversation: conversation.as_ref(),
    };
    output.print(&value, || {
        pull.print();
        if let Some(conversation) = &conversation {
            conversation.print();
        }
    })
}

pub(super) fn diff(api: &Api, repo: &Repo, args: DiffArgs) -> Result<()> {
    let DiffArgs { selector } = args;
    let number = pr::select(api, repo, selector.as_deref())?;
    print!("{}", pr::diff(api, repo, number)?);
    Ok(())
}

pub(super) fn checks(api: &Api, repo: &Repo, selection: Selection) -> Result<()> {
    let number = select_read(
        api,
        repo,
        selection.selector.as_deref(),
        &selection.output,
        &["name", "state", "description", "link"],
    )?;
    let pull = pr::get(api, repo, number)?;
    let statuses = pr::statuses(api, repo, &pull.head.sha)?;
    let values: Vec<_> = statuses
        .iter()
        .map(|status| Check {
            name: &status.context,
            state: &status.state,
            description: status.description.as_deref(),
            link: status.target_url.as_deref(),
        })
        .collect();
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

pub(super) fn browse(api: &Api, repo: &Repo, args: BrowseArgs) -> Result<()> {
    tui::browse_pull_requests(api, repo, args.state.as_str(), args.limit, &args.output)
}

#[derive(Serialize)]
struct PullView<'a> {
    #[serde(flatten)]
    pull: serde_json::Value,
    #[serde(flatten)]
    conversation: Option<&'a pr::Conversation>,
}

#[derive(Serialize)]
struct Check<'a> {
    name: &'a str,
    state: &'a str,
    description: Option<&'a str>,
    link: Option<&'a str>,
}
