use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail, ensure};
use clap::{Args, Subcommand};
use reqwest::Method;
use serde_json::{Value, json};

use crate::{client::Api, output::Output, repo::Repo, ui};

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
    limit: usize,
    /// Filter by Forgejo Actions status.
    #[arg(long)]
    status: Option<String>,
    /// Filter by git ref, such as refs/heads/main.
    #[arg(long)]
    r#ref: Option<String>,
    #[command(flatten)]
    output: Output,
}

#[derive(Args, Debug)]
pub struct RunId {
    /// Forgejo workflow run ID.
    id: u64,
    #[command(flatten)]
    output: Output,
}

#[derive(Args, Debug)]
pub struct Watch {
    /// Forgejo workflow run ID.
    id: u64,
    /// Seconds between polls.
    #[arg(long, default_value_t = 5, value_parser = clap::value_parser!(u64).range(1..))]
    interval: u64,
    /// Maximum seconds to wait, excluding an in-flight request.
    #[arg(long, default_value_t = 600, value_parser = clap::value_parser!(u64).range(1..))]
    timeout: u64,
    #[command(flatten)]
    output: Output,
}

#[derive(Args, Debug)]
pub struct Logs {
    /// Forgejo workflow run ID.
    id: u64,
    /// Only include jobs with failure status.
    #[arg(long)]
    failed: bool,
    #[command(flatten)]
    output: Output,
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
    output: Output,
}

#[derive(Args, Debug)]
pub struct WorkflowView {
    /// Path or filename under .forgejo/workflows or .github/workflows.
    workflow: String,
    #[command(flatten)]
    output: Output,
}

#[derive(Args, Debug)]
pub struct Dispatch {
    /// Workflow filename or path under .forgejo/workflows or .github/workflows.
    workflow: String,
    /// Git ref to dispatch.
    #[arg(long)]
    r#ref: String,
    /// Workflow input as NAME=VALUE; repeat for multiple inputs.
    #[arg(short = 'f', long = "field", value_name = "NAME=VALUE")]
    fields: Vec<String>,
    /// Include the created run information in Forgejo's response.
    #[arg(long)]
    return_run_info: bool,
    #[command(flatten)]
    output: Output,
}

pub fn run_run(command: RunCommand, api: &Api, repo: &Repo) -> Result<()> {
    match command {
        RunCommand::List(args) => {
            args.output.validate_fields(&[
                "id",
                "index_in_repo",
                "workflow_id",
                "status",
                "event",
                "title",
                "commit_sha",
                "prettyref",
                "created",
                "started",
                "stopped",
                "html_url",
                "trigger_event",
            ])?;
            ensure!(args.limit > 0, "list limit must be positive");
            let mut query = Vec::new();
            if let Some(status) = args.status {
                ensure!(
                    valid_status(&status),
                    "unsupported workflow run status: {status}"
                );
                query.push(("status".to_owned(), status));
            }
            if let Some(reference) = args.r#ref {
                ensure!(!reference.trim().is_empty(), "ref must not be empty");
                query.push(("ref".to_owned(), reference));
            }
            let mut runs = Vec::new();
            for page in 1..=100 {
                let mut page_query = query.clone();
                page_query.extend([
                    ("page".into(), page.to_string()),
                    ("limit".into(), "50".into()),
                ]);
                let response: Value =
                    api.json(Method::GET, &actions_runs_path(repo), &page_query, None)?;
                ensure!(
                    response
                        .get("total_count")
                        .and_then(Value::as_u64)
                        .is_some(),
                    "Forgejo action run response has no numeric total_count"
                );
                let batch = response
                    .get("workflow_runs")
                    .and_then(Value::as_array)
                    .context("Forgejo action run response has no workflow_runs array")?;
                if batch.is_empty() {
                    break;
                }
                for run in batch {
                    runs.push(run_summary(run)?);
                }
                if runs.len() >= args.limit {
                    runs.truncate(args.limit);
                    break;
                }
                if page == 100 {
                    bail!("pagination exceeded 100 pages; narrow the query or lower --limit");
                }
            }
            args.output.print(&runs, || {
                ui::heading("Workflow runs");
                ui::table(
                    &["Run", "Status", "Event", "Ref", "Title"],
                    &runs
                        .iter()
                        .map(|run| {
                            vec![
                                field(run, "id"),
                                field(run, "status"),
                                field(run, "event"),
                                field(run, "prettyref"),
                                field(run, "title"),
                            ]
                        })
                        .collect::<Vec<_>>(),
                );
            })
        }
        RunCommand::View(args) => {
            args.output.validate_fields(&run_fields())?;
            let path = run_path(repo, args.id);
            let value: Value = api.json(
                Method::GET,
                &path.iter().map(String::as_str).collect::<Vec<_>>(),
                &[],
                None,
            )?;
            validate_run(&value)?;
            args.output.print(&value, || {
                ui::heading(&format!("Workflow run {}", field(&value, "id")));
                ui::table(
                    &["Field", "Value"],
                    &run_fields()
                        .iter()
                        .filter_map(|key| {
                            value
                                .get(key)
                                .map(|value| vec![(*key).to_string(), display(value)])
                        })
                        .collect::<Vec<_>>(),
                );
            })
        }
        RunCommand::Jobs(args) => {
            args.output.validate_fields(&[
                "id", "run_id", "name", "status", "attempt", "task_id", "needs", "runs_on",
            ])?;
            let path = run_jobs_path(repo, args.id);
            let jobs: Vec<Value> = api.json(
                Method::GET,
                &path.iter().map(String::as_str).collect::<Vec<_>>(),
                &[],
                None,
            )?;
            args.output.print(&jobs, || {
                ui::heading(&format!("Jobs for run {}", args.id));
                ui::table(
                    &["Job", "Status", "Conclusion", "Runner"],
                    &jobs
                        .iter()
                        .map(|job| {
                            vec![
                                field(job, "name"),
                                field(job, "status"),
                                field(job, "conclusion"),
                                field(job, "runner_name"),
                            ]
                        })
                        .collect::<Vec<_>>(),
                );
            })
        }
        RunCommand::Watch(args) => {
            args.output.validate_fields(&run_fields())?;
            let start = Instant::now();
            let timeout = Duration::from_secs(args.timeout);
            let path = run_path(repo, args.id);
            loop {
                let value: Value = api.json(
                    Method::GET,
                    &path.iter().map(String::as_str).collect::<Vec<_>>(),
                    &[],
                    None,
                )?;
                validate_run(&value)?;
                let status = value["status"]
                    .as_str()
                    .context("workflow run has no status")?;
                ensure!(
                    valid_status(status),
                    "unsupported workflow run status: {status}"
                );
                if matches!(status, "success" | "failure" | "cancelled" | "skipped") {
                    args.output.print(&value, || {
                        ui::heading(&format!("Workflow run {}", args.id));
                        ui::status(
                            "Status",
                            status,
                            if status == "success" {
                                ui::Status::Success
                            } else {
                                ui::Status::Error
                            },
                        );
                    })?;
                    ensure!(
                        status == "success",
                        "workflow run {} finished with {status}",
                        args.id
                    );
                    return Ok(());
                }
                let remaining = timeout
                    .checked_sub(start.elapsed())
                    .context("timed out waiting for workflow run")?;
                eprintln!("Run {}: {status}; waiting...", args.id);
                std::thread::sleep(Duration::from_secs(args.interval).min(remaining));
                ensure!(
                    start.elapsed() < timeout,
                    "timed out waiting for workflow run"
                );
            }
        }
        RunCommand::Logs(args) => {
            args.output
                .validate_fields(&["id", "name", "status", "attempt", "log"])?;
            let path = run_jobs_path(repo, args.id);
            let jobs: Vec<Value> = api.json(
                Method::GET,
                &path.iter().map(String::as_str).collect::<Vec<_>>(),
                &[],
                None,
            )?;
            let mut logs = Vec::new();
            for job in jobs {
                let status = job["status"]
                    .as_str()
                    .context("workflow job has no status")?;
                if args.failed && status != "failure" {
                    continue;
                }
                let id = job["id"]
                    .as_u64()
                    .context("workflow job has no numeric ID")?
                    .to_string();
                let mut path = repo.path().to_vec();
                path.extend(["actions", "jobs", &id, "logs"]);
                let mut query = Vec::new();
                if let Some(attempt) = job["attempt"].as_u64() {
                    query.push(("attempt".into(), attempt.to_string()));
                }
                let log = api.text(Method::GET, &path, &query, None)?;
                logs.push(json!({"id": job["id"], "name": job["name"], "status": status, "attempt": job["attempt"], "log": log}));
            }
            args.output.print(&logs, || {
                if logs.is_empty() {
                    println!("No matching jobs.");
                }
                for job in &logs {
                    ui::heading(&format!(
                        "{} ({})",
                        field(job, "name"),
                        field(job, "status")
                    ));
                    ui::markdown(job["log"].as_str().unwrap_or(""));
                }
            })
        }
        RunCommand::Delete(args) => {
            args.output.validate_fields(&["id", "deleted"])?;
            let path = run_path(repo, args.id);
            api.text(
                Method::DELETE,
                &path.iter().map(String::as_str).collect::<Vec<_>>(),
                &[],
                None,
            )?;
            let value = json!({"id": args.id, "deleted": true});
            args.output.print(&value, || {
                println!("Deleted completed workflow run {}.", args.id)
            })
        }
    }
}

pub fn run_workflow(command: WorkflowCommand, api: &Api, repo: &Repo) -> Result<()> {
    match command {
        WorkflowCommand::List(args) => {
            args.output
                .validate_fields(&["name", "path", "html_url", "sha", "type"])?;
            let repo_path = repo.path();
            let _: Value = api
                .json(Method::GET, &repo_path, &[], None)
                .context("verify repository before reading workflow directories")?;
            let mut workflows = Vec::new();
            for directory in [".forgejo/workflows", ".github/workflows"] {
                let path = contents_path(repo, directory);
                match api.json::<Value>(
                    Method::GET,
                    &path.iter().map(String::as_str).collect::<Vec<_>>(),
                    &[],
                    None,
                ) {
                    Ok(Value::Array(entries)) => {
                        workflows.extend(entries.into_iter().filter(is_workflow_file))
                    }
                    Ok(_) => bail!("Forgejo workflow directory response is not an array"),
                    Err(error) if is_missing_workflow_directory(&error.to_string(), directory) => {}
                    Err(error) => return Err(error),
                }
            }
            args.output.print(&workflows, || {
                ui::heading("Workflow files");
                ui::table(
                    &["Name", "Path", "Type", "URL"],
                    &workflows
                        .iter()
                        .map(|workflow| {
                            vec![
                                field(workflow, "name"),
                                field(workflow, "path"),
                                field(workflow, "type"),
                                field(workflow, "html_url"),
                            ]
                        })
                        .collect::<Vec<_>>(),
                );
            })
        }
        WorkflowCommand::View(args) => {
            args.output.validate_fields(&[
                "name", "path", "html_url", "sha", "type", "encoding", "size",
            ])?;
            let path = contents_path(repo, &workflow_path(&args.workflow)?);
            let value: Value = api.json(
                Method::GET,
                &path.iter().map(String::as_str).collect::<Vec<_>>(),
                &[],
                None,
            )?;
            ensure!(is_workflow_file(&value), "path is not a YAML workflow file");
            args.output.print(&value, || {
                ui::heading(&field(&value, "name"));
                ui::detail("Path", &field(&value, "path"));
                ui::detail("Type", &field(&value, "type"));
                ui::detail("URL", &field(&value, "html_url"));
                ui::detail("Size", &field(&value, "size"));
            })
        }
        WorkflowCommand::Dispatch(args) => {
            args.output
                .validate_fields(&["id", "run_number", "jobs", "dispatched"])?;
            let workflow = workflow_path(&args.workflow)?;
            ensure!(!args.r#ref.trim().is_empty(), "ref must not be empty");
            api.require_token()?;
            let mut inputs = serde_json::Map::new();
            for field in args.fields {
                let (name, value) = field
                    .split_once('=')
                    .context("workflow input must use NAME=VALUE")?;
                ensure!(
                    !name.trim().is_empty(),
                    "workflow input name must be nonempty"
                );
                ensure!(
                    !inputs.contains_key(name),
                    "duplicate workflow input {name}"
                );
                inputs.insert(name.to_owned(), Value::String(value.to_owned()));
            }
            let body = json!({"ref": args.r#ref, "inputs": inputs, "return_run_info": args.return_run_info});
            let filename = workflow
                .rsplit('/')
                .next()
                .expect("validated workflow path has a filename");
            let path = dispatch_path(repo, filename);
            let response = api.text(
                Method::POST,
                &path.iter().map(String::as_str).collect::<Vec<_>>(),
                &[],
                Some(&body),
            )?;
            let value: Value = if response.is_empty() {
                json!({"dispatched": true})
            } else {
                let mut response: Value =
                    serde_json::from_str(&response).context("decode workflow dispatch response")?;
                let object = response
                    .as_object_mut()
                    .context("Forgejo workflow dispatch response must be an object")?;
                object.insert("dispatched".into(), json!(true));
                response
            };
            args.output.print(&value, || {
                ui::status(
                    "Workflow",
                    &format!("Dispatched {workflow} on {}", field(&body, "ref")),
                    ui::Status::Success,
                )
            })
        }
    }
}

fn field(value: &Value, key: &str) -> String {
    value.get(key).map(display).unwrap_or_default()
}

fn display(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| match value {
            Value::Null => String::new(),
            _ => value.to_string(),
        })
}

fn actions_runs_path(repo: &Repo) -> Vec<&str> {
    let p = repo.path();
    vec![p[0], p[1], p[2], "actions", "runs"]
}
fn run_path(repo: &Repo, id: u64) -> Vec<String> {
    vec![
        "repos".into(),
        repo.owner.clone(),
        repo.name.clone(),
        "actions".into(),
        "runs".into(),
        id.to_string(),
    ]
}
fn run_jobs_path(repo: &Repo, id: u64) -> Vec<String> {
    vec![
        "repos".into(),
        repo.owner.clone(),
        repo.name.clone(),
        "actions".into(),
        "runs".into(),
        id.to_string(),
        "jobs".into(),
    ]
}
fn contents_path(repo: &Repo, path: &str) -> Vec<String> {
    vec![
        "repos".into(),
        repo.owner.clone(),
        repo.name.clone(),
        "contents".into(),
        path.into(),
    ]
}
fn dispatch_path(repo: &Repo, workflow: &str) -> Vec<String> {
    vec![
        "repos".into(),
        repo.owner.clone(),
        repo.name.clone(),
        "actions".into(),
        "workflows".into(),
        workflow.into(),
        "dispatches".into(),
    ]
}
fn run_fields() -> Vec<&'static str> {
    vec![
        "id",
        "index_in_repo",
        "workflow_id",
        "status",
        "event",
        "title",
        "commit_sha",
        "prettyref",
        "created",
        "started",
        "stopped",
        "html_url",
        "trigger_event",
    ]
}
fn valid_status(value: &str) -> bool {
    matches!(
        value,
        "unknown"
            | "waiting"
            | "running"
            | "success"
            | "failure"
            | "cancelled"
            | "skipped"
            | "blocked"
    )
}
fn validate_run(value: &Value) -> Result<()> {
    let object = value
        .as_object()
        .context("Forgejo action run is not an object")?;
    ensure!(
        object.get("id").and_then(Value::as_u64).is_some(),
        "Forgejo action run has no numeric id"
    );
    ensure!(
        object.get("status").and_then(Value::as_str).is_some(),
        "Forgejo action run has no status"
    );
    ensure!(
        object.get("commit_sha").and_then(Value::as_str).is_some(),
        "Forgejo action run has no commit_sha"
    );
    Ok(())
}

fn run_summary(value: &Value) -> Result<Value> {
    validate_run(value)?;
    let fields = [
        "id",
        "index_in_repo",
        "workflow_id",
        "status",
        "event",
        "title",
        "commit_sha",
        "prettyref",
        "created",
        "started",
        "stopped",
        "html_url",
        "trigger_event",
    ];
    let mut summary = serde_json::Map::new();
    for field in fields {
        if let Some(value) = value.get(field) {
            summary.insert(field.into(), value.clone());
        }
    }
    Ok(Value::Object(summary))
}

fn is_missing_workflow_directory(error: &str, directory: &str) -> bool {
    error.contains("HTTP 404") && error.contains(&format!("contents/{directory}"))
}

fn is_workflow_file(value: &Value) -> bool {
    let path = value.get("path").and_then(Value::as_str).unwrap_or("");
    let name = value.get("name").and_then(Value::as_str).unwrap_or(path);
    matches!(name.rsplit('.').next(), Some("yml" | "yaml"))
}
fn workflow_path(value: &str) -> Result<String> {
    let value = value.trim_start_matches('/');
    let value =
        if value.starts_with(".forgejo/workflows/") || value.starts_with(".github/workflows/") {
            value.to_owned()
        } else {
            ensure!(
                !value.contains('/'),
                "workflow must be a filename or path under a workflow directory"
            );
            format!(".forgejo/workflows/{value}")
        };
    ensure!(
        value.ends_with(".yml") || value.ends_with(".yaml"),
        "workflow path must end in .yml or .yaml"
    );
    ensure!(
        !value.split('/').any(|part| part == ".." || part.is_empty()),
        "invalid workflow path"
    );
    Ok(value)
}
