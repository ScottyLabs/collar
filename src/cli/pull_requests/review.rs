use anyhow::{Context, Result, ensure};

use super::super::input::read_file;
use super::super::pr_args::ReviewArgs;
use crate::{client::Api, pr, repo::Repo};

pub(super) fn review(api: &Api, repo: &Repo, args: ReviewArgs) -> Result<()> {
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
