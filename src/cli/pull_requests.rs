mod read;
mod review;
mod write;

use anyhow::Result;

use super::pr_args::PrCommand;
use crate::{client::Api, output::Output, pr, repo::Repo};

pub(super) fn run(command: PrCommand, api: &Api, repo: &Repo) -> Result<()> {
    match command {
        PrCommand::Checkout(args) => write::checkout(api, repo, args),
        PrCommand::Merge(args) => write::merge(api, repo, args),
        PrCommand::UpdateBranch(args) => write::update_branch(api, repo, args),
        PrCommand::Create(args) => write::create(api, repo, args),
        PrCommand::Edit(args) => write::edit(api, repo, args),
        PrCommand::Comment(args) => write::comment(api, repo, args),
        PrCommand::Close(selection) => write::set_state(api, repo, selection, "closed"),
        PrCommand::Reopen(selection) => write::set_state(api, repo, selection, "open"),
        PrCommand::Browse(args) => read::browse(api, repo, args),
        PrCommand::List(args) => read::list(api, repo, args),
        PrCommand::View(args) => read::view(api, repo, args),
        PrCommand::Diff(args) => read::diff(api, repo, args),
        PrCommand::Checks(selection) => read::checks(api, repo, selection),
        PrCommand::Review(args) => review::review(api, repo, args),
    }
}

fn prepare_write(api: &Api, output: &Output, fields: &[&str]) -> Result<()> {
    output.validate_fields(fields)?;
    api.require_token()
}

fn select_write(
    api: &Api,
    repo: &Repo,
    selector: Option<&str>,
    output: &Output,
    fields: &[&str],
) -> Result<u64> {
    prepare_write(api, output, fields)?;
    pr::select(api, repo, selector)
}

fn select_read(
    api: &Api,
    repo: &Repo,
    selector: Option<&str>,
    output: &Output,
    fields: &[&str],
) -> Result<u64> {
    output.validate_fields(fields)?;
    pr::select(api, repo, selector)
}
