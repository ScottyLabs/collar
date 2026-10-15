use anyhow::{Context, Result, ensure};
use serde::Serialize;
use serde_json::json;

use super::super::input::Selection;
use super::super::pr_args::{
    CheckoutArgs, CommentArgs, CreateArgs, EditArgs, MergeArgs, UpdateBranchArgs,
};
use super::{prepare_write, select_write};
use crate::{checkout, client::Api, pr, repo::Repo};

pub(super) fn merge(api: &Api, repo: &Repo, args: MergeArgs) -> Result<()> {
    let MergeArgs {
        selector,
        merge: _,
        squash,
        rebase,
        match_head_commit,
        delete_branch,
        auto,
        output,
    } = args;
    let method = if squash {
        "squash"
    } else if rebase {
        "rebase"
    } else {
        "merge"
    };
    let number = select_write(
        api,
        repo,
        selector.as_deref(),
        &output,
        &[
            "number",
            "headRefOid",
            "method",
            "autoMergeRequested",
            "accepted",
        ],
    )?;
    let pull = pr::open_head(api, repo, number, match_head_commit.as_deref())?;
    pr::merge(api, repo, &pull, method, delete_branch, auto)?;
    let value = MergeResult {
        number,
        head_ref_oid: &pull.head.sha,
        method,
        auto_merge_requested: auto,
        accepted: true,
    };
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

pub(super) fn update_branch(api: &Api, repo: &Repo, args: UpdateBranchArgs) -> Result<()> {
    let UpdateBranchArgs {
        selector,
        rebase,
        match_head_commit,
        output,
    } = args;
    let number = select_write(
        api,
        repo,
        selector.as_deref(),
        &output,
        &["number", "headRefOid", "method", "accepted"],
    )?;
    let pull = pr::open_head(api, repo, number, match_head_commit.as_deref())?;
    let style = if rebase { "rebase" } else { "merge" };
    pr::update_branch(api, repo, number, style)?;
    let value = UpdateBranchResult {
        number,
        head_ref_oid: &pull.head.sha,
        method: style,
        accepted: true,
    };
    output.print(&value, || {
        println!(
            "Forgejo accepted the {style} branch update for PR #{number}; inspected head {}",
            pull.head.sha
        )
    })
}

pub(super) fn create(api: &Api, repo: &Repo, args: CreateArgs) -> Result<()> {
    let CreateArgs {
        title,
        head,
        base,
        text,
        output,
    } = args;
    prepare_write(api, &output, pr::Pull::FIELDS)?;
    let body = text.read()?.unwrap_or_default();
    let head = head.map(Ok).unwrap_or_else(crate::repo::branch)?;
    let pull = pr::create(api, repo, &title, &head, base.as_deref(), &body)?;
    output.print(&pull.output(), || println!("{}", pull.html_url))
}

pub(super) fn edit(api: &Api, repo: &Repo, args: EditArgs) -> Result<()> {
    let EditArgs {
        selector,
        title,
        base,
        text,
        output,
    } = args;
    prepare_write(api, &output, pr::Pull::FIELDS)?;
    let body = text.read()?;
    for (key, value) in [("title", &title), ("base", &base)] {
        if let Some(value) = value {
            ensure!(!value.trim().is_empty(), "{key} must be nonempty");
        }
    }
    ensure!(
        title.is_some() || base.is_some() || body.is_some(),
        "provide --title, --base, --body, or --body-file"
    );
    let change = serde_json::to_value(PullEdit { title, base, body })?;
    let number = pr::select(api, repo, selector.as_deref())?;
    let pull = pr::edit(api, repo, number, &change)?;
    output.print(&pull.output(), || println!("{}", pull.html_url))
}

pub(super) fn comment(api: &Api, repo: &Repo, args: CommentArgs) -> Result<()> {
    let CommentArgs {
        selector,
        text,
        output,
    } = args;
    prepare_write(api, &output, &["id", "body", "url", "author"])?;
    let body = text.read()?.context("provide --body or --body-file")?;
    ensure!(!body.trim().is_empty(), "comment body must be nonempty");
    let number = pr::select(api, repo, selector.as_deref())?;
    let comment = pr::comment(api, repo, number, &body)?;
    output.print(&comment.output(), || println!("{}", comment.html_url))
}

pub(super) fn set_state(api: &Api, repo: &Repo, selection: Selection, state: &str) -> Result<()> {
    let number = select_write(
        api,
        repo,
        selection.selector.as_deref(),
        &selection.output,
        pr::Pull::FIELDS,
    )?;
    let current = pr::get(api, repo, number)?;
    ensure!(!current.merged, "a merged PR cannot be closed or reopened");
    ensure!(current.state != state, "PR is already {state}");
    let pull = pr::edit(api, repo, number, &json!({"state":state}))?;
    selection
        .output
        .print(&pull.output(), || println!("{}", pull.html_url))
}

pub(super) fn checkout(api: &Api, repo: &Repo, args: CheckoutArgs) -> Result<()> {
    checkout::run(
        api,
        repo,
        args.selector.as_deref(),
        args.branch.as_deref(),
        args.remote.as_deref(),
    )
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MergeResult<'a> {
    number: u64,
    head_ref_oid: &'a str,
    method: &'a str,
    auto_merge_requested: bool,
    accepted: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateBranchResult<'a> {
    number: u64,
    head_ref_oid: &'a str,
    method: &'a str,
    accepted: bool,
}

#[derive(Serialize)]
struct PullEdit {
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    base: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<String>,
}
