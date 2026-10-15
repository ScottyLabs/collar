use clap::{Args, Subcommand};

use crate::output::Output;

#[derive(Subcommand, Debug)]
pub enum RunCommand {
    /// List workflow runs.
    List(ListRuns),
    /// View one workflow run.
    View(RunId),
    /// List jobs for a workflow run.
    Jobs(RunId),
    /// Wait for a workflow run; fail if it does not succeed.
    Watch(Watch),
    /// Print job logs for a run, optionally only failed jobs.
    Logs(Logs),
    /// Delete a completed workflow run.
    Delete(RunId),
}

#[derive(Args, Debug)]
pub struct ListRuns {
    /// Maximum number of runs to return.
    #[arg(long, default_value_t = 50)]
    pub(super) limit: usize,
    /// Filter by Forgejo Actions status.
    #[arg(long)]
    pub(super) status: Option<String>,
    /// Filter by git ref, such as refs/heads/main.
    #[arg(long)]
    pub(super) r#ref: Option<String>,
    #[command(flatten)]
    pub(super) output: Output,
}

#[derive(Args, Debug)]
pub struct RunId {
    /// Forgejo workflow run ID.
    pub(super) id: u64,
    #[command(flatten)]
    pub(super) output: Output,
}

#[derive(Args, Debug)]
pub struct Watch {
    /// Forgejo workflow run ID.
    pub(super) id: u64,
    /// Seconds between polls.
    #[arg(long, default_value_t = 5, value_parser = clap::value_parser!(u64).range(1..))]
    pub(super) interval: u64,
    /// Maximum seconds to wait, excluding an in-flight request.
    #[arg(long, default_value_t = 600, value_parser = clap::value_parser!(u64).range(1..))]
    pub(super) timeout: u64,
    #[command(flatten)]
    pub(super) output: Output,
}

#[derive(Args, Debug)]
pub struct Logs {
    /// Forgejo workflow run ID.
    pub(super) id: u64,
    /// Only include jobs with failure status.
    #[arg(long)]
    pub(super) failed: bool,
    #[command(flatten)]
    pub(super) output: Output,
}

#[derive(Subcommand, Debug)]
pub enum WorkflowCommand {
    /// List workflow files in .forgejo/workflows and .github/workflows.
    List(WorkflowList),
    /// View workflow file metadata returned by Forgejo.
    View(WorkflowView),
    /// Dispatch an existing workflow with optional NAME=VALUE inputs.
    Dispatch(Dispatch),
}

#[derive(Args, Debug)]
pub struct WorkflowList {
    #[command(flatten)]
    pub(super) output: Output,
}

#[derive(Args, Debug)]
pub struct WorkflowView {
    /// Path or filename under .forgejo/workflows or .github/workflows.
    pub(super) workflow: String,
    #[command(flatten)]
    pub(super) output: Output,
}

#[derive(Args, Debug)]
pub struct Dispatch {
    /// Workflow filename or path under .forgejo/workflows or .github/workflows.
    pub(super) workflow: String,
    /// Git ref to dispatch.
    #[arg(long)]
    pub(super) r#ref: String,
    /// Workflow input as NAME=VALUE; repeat for multiple inputs.
    #[arg(short = 'f', long = "field", value_name = "NAME=VALUE")]
    pub(super) fields: Vec<String>,
    /// Include the created run information in Forgejo's response.
    #[arg(long)]
    pub(super) return_run_info: bool,
    #[command(flatten)]
    pub(super) output: Output,
}
