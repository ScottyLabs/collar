use std::path::PathBuf;

use clap::{ArgGroup, Args, Subcommand, ValueEnum};

use super::input::{PullText, Selection};
use crate::output::Output;

#[derive(Subcommand)]
pub(super) enum PrCommand {
    /// Fetch a verified PR head and switch to a local branch.
    Checkout {
        /// PR number, same-repository URL, or branch; defaults to the current branch.
        selector: Option<String>,
        /// Local branch name; defaults to pr/NUMBER.
        #[arg(short = 'b', long)]
        branch: Option<String>,
        /// Configured Git remote; defaults to upstream, then origin.
        #[arg(long)]
        remote: Option<String>,
    },
    /// Merge the inspected PR head using Forgejo protection checks.
    #[command(group(ArgGroup::new("strategy").required(true).args(["merge", "squash", "rebase"])))]
    Merge {
        /// PR number, same-repository URL, or branch; defaults to the current branch.
        selector: Option<String>,
        /// Preserve commits with a merge commit.
        #[arg(long)]
        merge: bool,
        /// Combine the PR into a single commit.
        #[arg(long)]
        squash: bool,
        /// Rebase the PR commits instead of creating a merge commit.
        #[arg(long)]
        rebase: bool,
        /// Refuse if the current head differs from this commit.
        #[arg(long)]
        match_head_commit: Option<String>,
        /// Ask Forgejo to delete the source branch after merging.
        #[arg(long)]
        delete_branch: bool,
        /// Ask Forgejo to merge when its checks succeed.
        #[arg(long, conflicts_with = "match_head_commit")]
        auto: bool,
        #[command(flatten)]
        output: Output,
    },
    /// Merge or rebase the target branch into the PR branch.
    UpdateBranch {
        /// PR number, same-repository URL, or branch; defaults to the current branch.
        selector: Option<String>,
        /// Rebase the PR commits instead of creating a merge commit.
        #[arg(long)]
        rebase: bool,
        /// Check the current head before submission; the update API has no atomic guard.
        #[arg(long)]
        match_head_commit: Option<String>,
        #[command(flatten)]
        output: Output,
    },
    /// Create a PR from an already-pushed branch.
    Create {
        /// Pull request title.
        #[arg(short = 't', long)]
        title: String,
        /// Branch or fork-owner:branch; defaults to the current Git branch.
        #[arg(short = 'H', long)]
        head: Option<String>,
        /// Target branch; defaults to the forge repository's default branch.
        #[arg(short = 'B', long)]
        base: Option<String>,
        #[command(flatten)]
        text: PullText,
        #[command(flatten)]
        output: Output,
    },
    /// Edit a PR's title, body, or target branch.
    Edit {
        /// PR number, same-repository URL, or branch; defaults to the current branch.
        selector: Option<String>,
        /// Pull request title.
        #[arg(short = 't', long)]
        title: Option<String>,
        #[arg(short = 'B', long)]
        base: Option<String>,
        #[command(flatten)]
        text: PullText,
        #[command(flatten)]
        output: Output,
    },
    /// Add a discussion comment to a PR.
    Comment {
        /// PR number, same-repository URL, or branch; defaults to the current branch.
        selector: Option<String>,
        #[command(flatten)]
        text: PullText,
        #[command(flatten)]
        output: Output,
    },
    /// Close a PR without merging it.
    Close(Selection),
    /// Reopen a closed, unmerged PR.
    Reopen(Selection),
    /// Select a PR interactively, then inspect its details, diff, or checks.
    Browse {
        #[arg(short = 's', long, value_enum, default_value = "open")]
        state: State,
        #[arg(short = 'L', long, default_value_t = 30)]
        limit: usize,
        #[command(flatten)]
        output: Output,
    },
    /// List pull requests.
    List {
        #[arg(short = 's', long, value_enum, default_value = "open")]
        state: State,
        #[arg(short = 'L', long, default_value_t = 30)]
        limit: usize,
        /// Filter by author login; @me uses the authenticated account.
        #[arg(long)]
        author: Option<String>,
        /// Filter by target branch.
        #[arg(long)]
        base: Option<String>,
        /// Filter by source branch.
        #[arg(long)]
        head: Option<String>,
        #[command(flatten)]
        output: Output,
    },
    /// View a PR selected by number, URL, or branch (default: current branch).
    View {
        /// PR number, same-repository URL, or branch; defaults to the current branch.
        selector: Option<String>,
        /// Include discussion, reviews, and inline review comments.
        #[arg(long)]
        comments: bool,
        #[command(flatten)]
        output: Output,
    },
    /// Print a PR's unified diff.
    Diff { selector: Option<String> },
    /// Show commit statuses; fail when none are reported or any are not successful.
    Checks(Selection),
    /// Submit a review, optionally including inline comments.
    #[command(
        after_long_help = "Examples:\n  collar pr review 42 --approve\n  collar pr review 42 --request-changes --body 'Cover the timeout case.'\n  collar pr review 42 --comment --inline-comment src/main.rs 12 new 'Handle this error.'\n\nSELECTOR is a PR number, same-repository URL, or branch; omitted selects the current branch. Reviews submit immediately and are pinned to the inspected head."
    )]
    Review(ReviewArgs),
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
