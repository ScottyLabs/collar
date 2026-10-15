use std::path::PathBuf;

use clap::{ArgGroup, Args, Subcommand, ValueEnum};

use super::input::{PullText, Selection};
use crate::output::Output;

#[derive(Subcommand)]
pub(super) enum PrCommand {
    /// Fetch a verified PR head and switch to a local branch.
    Checkout(CheckoutArgs),
    /// Merge the inspected PR head using Forgejo protection checks.
    #[command(group(ArgGroup::new("strategy").required(true).args(["merge", "squash", "rebase"])))]
    Merge(MergeArgs),
    /// Merge or rebase the target branch into the PR branch.
    UpdateBranch(UpdateBranchArgs),
    /// Create a PR from an already-pushed branch.
    Create(CreateArgs),
    /// Edit a PR's title, body, or target branch.
    Edit(EditArgs),
    /// Add a discussion comment to a PR.
    Comment(CommentArgs),
    /// Close a PR without merging it.
    Close(Selection),
    /// Reopen a closed, unmerged PR.
    Reopen(Selection),
    /// Select a PR interactively, then inspect its details, diff, or checks.
    Browse(BrowseArgs),
    /// List pull requests.
    List(ListArgs),
    /// View a PR selected by number, URL, or branch (default: current branch).
    View(ViewArgs),
    /// Print a PR's unified diff.
    Diff(DiffArgs),
    /// Show commit statuses; fail when none are reported or any are not successful.
    Checks(Selection),
    /// Submit a review, optionally including inline comments.
    #[command(
        after_long_help = "Examples:\n  collar pr review 42 --approve\n  collar pr review 42 --request-changes --body 'Cover the timeout case.'\n  collar pr review 42 --comment --inline-comment src/main.rs 12 new 'Handle this error.'\n\nSELECTOR is a PR number, same-repository URL, or branch; omitted selects the current branch. Reviews submit immediately and are pinned to the inspected head."
    )]
    Review(ReviewArgs),
    /// List, request, or cancel reviewers for a PR.
    #[command(
        after_long_help = "Examples:\n  collar pr reviewers 42\n  collar pr reviewers 42 --add anish kritdass\n  collar pr reviewers 42 --remove anish\n\nWith no changes, lists the requested users and teams. Changes require sign-in and an open PR; --add and --remove cannot be combined."
    )]
    Reviewers(ReviewersArgs),
}

#[derive(Args)]
pub(super) struct ReviewersArgs {
    /// PR number, URL, or branch; defaults to the current branch.
    pub(super) selector: Option<String>,
    /// Request reviews from these usernames.
    #[arg(long, num_args = 1.., conflicts_with = "remove")]
    pub(super) add: Vec<String>,
    /// Cancel review requests for these usernames.
    #[arg(long, num_args = 1..)]
    pub(super) remove: Vec<String>,
    #[command(flatten)]
    pub(super) output: Output,
}

#[derive(Clone, ValueEnum)]
pub(super) enum State {
    Open,
    Closed,
    All,
}

impl State {
    pub(super) fn as_str(&self) -> &str {
        match self {
            Self::Open => "open",
            Self::Closed => "closed",
            Self::All => "all",
        }
    }
}

#[derive(Args)]
#[command(group(ArgGroup::new("verdict").required(true).args(["approve", "request_changes", "comment"])))]
pub(super) struct ReviewArgs {
    /// PR number, same-repository URL, or branch; defaults to the current branch.
    pub(super) selector: Option<String>,
    /// Submit an approval.
    #[arg(short = 'a', long)]
    pub(super) approve: bool,
    /// Submit a change request.
    #[arg(short = 'r', long)]
    pub(super) request_changes: bool,
    /// Submit a review comment without approving or requesting changes.
    #[arg(short = 'c', long)]
    pub(super) comment: bool,
    /// Markdown text for the description, discussion comment, or review.
    #[arg(short = 'b', long, conflicts_with = "body_file")]
    pub(super) body: Option<String>,
    /// Read review text from a file, or - for stdin.
    #[arg(short = 'F', long)]
    pub(super) body_file: Option<PathBuf>,
    /// Add an inline comment; repeat for multiple comments. SIDE is old or new.
    #[arg(long = "inline-comment", num_args = 4, value_names = ["PATH", "LINE", "SIDE", "BODY"], action = clap::ArgAction::Append)]
    pub(super) inline_comments: Vec<String>,
}

#[derive(Args)]
pub(super) struct CheckoutArgs {
    /// PR number, same-repository URL, or branch; defaults to the current branch.
    pub(super) selector: Option<String>,
    /// Local branch name; defaults to pr/NUMBER.
    #[arg(short = 'b', long)]
    pub(super) branch: Option<String>,
    /// Configured Git remote; defaults to upstream, then origin.
    #[arg(long)]
    pub(super) remote: Option<String>,
}

#[derive(Args)]
pub(super) struct MergeArgs {
    /// PR number, same-repository URL, or branch; defaults to the current branch.
    pub(super) selector: Option<String>,
    /// Preserve commits with a merge commit.
    #[arg(long)]
    pub(super) merge: bool,
    /// Combine the PR into a single commit.
    #[arg(long)]
    pub(super) squash: bool,
    /// Rebase the PR commits instead of creating a merge commit.
    #[arg(long)]
    pub(super) rebase: bool,
    /// Refuse if the current head differs from this commit.
    #[arg(long)]
    pub(super) match_head_commit: Option<String>,
    /// Ask Forgejo to delete the source branch after merging.
    #[arg(long)]
    pub(super) delete_branch: bool,
    /// Ask Forgejo to merge when its checks succeed.
    #[arg(long, conflicts_with = "match_head_commit")]
    pub(super) auto: bool,
    #[command(flatten)]
    pub(super) output: Output,
}

#[derive(Args)]
pub(super) struct UpdateBranchArgs {
    /// PR number, same-repository URL, or branch; defaults to the current branch.
    pub(super) selector: Option<String>,
    /// Rebase the PR commits instead of creating a merge commit.
    #[arg(long)]
    pub(super) rebase: bool,
    /// Check the current head before submission; the update API has no atomic guard.
    #[arg(long)]
    pub(super) match_head_commit: Option<String>,
    #[command(flatten)]
    pub(super) output: Output,
}

#[derive(Args)]
pub(super) struct CreateArgs {
    /// Pull request title.
    #[arg(short = 't', long)]
    pub(super) title: String,
    /// Branch or fork-owner:branch; defaults to the current Git branch.
    #[arg(short = 'H', long)]
    pub(super) head: Option<String>,
    /// Target branch; defaults to the forge repository's default branch.
    #[arg(short = 'B', long)]
    pub(super) base: Option<String>,
    #[command(flatten)]
    pub(super) text: PullText,
    #[command(flatten)]
    pub(super) output: Output,
}

#[derive(Args)]
pub(super) struct EditArgs {
    /// PR number, same-repository URL, or branch; defaults to the current branch.
    pub(super) selector: Option<String>,
    /// Pull request title.
    #[arg(short = 't', long)]
    pub(super) title: Option<String>,
    #[arg(short = 'B', long)]
    pub(super) base: Option<String>,
    #[command(flatten)]
    pub(super) text: PullText,
    #[command(flatten)]
    pub(super) output: Output,
}

#[derive(Args)]
pub(super) struct CommentArgs {
    /// PR number, same-repository URL, or branch; defaults to the current branch.
    pub(super) selector: Option<String>,
    #[command(flatten)]
    pub(super) text: PullText,
    #[command(flatten)]
    pub(super) output: Output,
}

#[derive(Args)]
pub(super) struct BrowseArgs {
    #[arg(short = 's', long, value_enum, default_value = "open")]
    pub(super) state: State,
    #[arg(short = 'L', long, default_value_t = 30)]
    pub(super) limit: usize,
    #[command(flatten)]
    pub(super) output: Output,
}

#[derive(Args)]
pub(super) struct ListArgs {
    #[arg(short = 's', long, value_enum, default_value = "open")]
    pub(super) state: State,
    #[arg(short = 'L', long, default_value_t = 30)]
    pub(super) limit: usize,
    /// Filter by author login; @me uses the authenticated account.
    #[arg(long)]
    pub(super) author: Option<String>,
    /// Filter by target branch.
    #[arg(long)]
    pub(super) base: Option<String>,
    /// Filter by source branch.
    #[arg(long)]
    pub(super) head: Option<String>,
    #[command(flatten)]
    pub(super) output: Output,
}

#[derive(Args)]
pub(super) struct ViewArgs {
    /// PR number, same-repository URL, or branch; defaults to the current branch.
    pub(super) selector: Option<String>,
    /// Include discussion, reviews, and inline review comments.
    #[arg(long)]
    pub(super) comments: bool,
    /// Omit provider metadata from conversation JSON.
    #[arg(long, requires_all = ["comments", "json"])]
    pub(super) compact: bool,
    #[command(flatten)]
    pub(super) output: Output,
}

#[derive(Args)]
pub(super) struct DiffArgs {
    pub(super) selector: Option<String>,
}
