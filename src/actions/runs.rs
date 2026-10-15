use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail, ensure};
use reqwest::Method;
use serde_json::json;

use super::commands::{ListRuns, Logs, RunCommand, RunId, Watch};
use super::models::{Job, JobLog, RUN_FIELDS, Run, RunList, RunRecord, Status, display};
use crate::{client::Api, repo::Repo, ui};

pub fn run_run(command: RunCommand, api: &Api, repo: &Repo) -> Result<()> {
    match command {
        RunCommand::List(args) => list(args, api, repo),
        RunCommand::View(args) => view(args, api, repo),
        RunCommand::Jobs(args) => jobs(args, api, repo),
        RunCommand::Watch(args) => watch(args, api, repo),
        RunCommand::Logs(args) => logs(args, api, repo),
        RunCommand::Delete(args) => {
            args.output.validate_fields(&["id", "deleted"])?;
            let id = args.id.to_string();
            let mut path = runs_path(repo);
            path.push(&id);
            api.text(Method::DELETE, &path, &[], None)?;
            args.output
                .print(&json!({"id": args.id, "deleted": true}), || {
                    println!("Deleted completed workflow run {}.", args.id)
                })
        }
    }
}

fn list(args: ListRuns, api: &Api, repo: &Repo) -> Result<()> {
    args.output.validate_fields(RUN_FIELDS)?;
    ensure!(args.limit > 0, "list limit must be positive");
    let mut query = Vec::new();
    if let Some(status) = args.status {
        ensure!(
            Status::parse(&status).is_known(),
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
        let response: RunList = api.json(Method::GET, &runs_path(repo), &page_query, None)?;
        // Follow pages rather than a count that can change as new runs arrive.
        let records = response.into_records()?;
        if records.is_empty() {
            break;
        }
        for run in records {
            runs.push(Run::try_from(run)?);
        }
        if runs.len() >= args.limit {
            runs.truncate(args.limit);
            break;
        }
        if page == 100 {
            bail!("pagination exceeded 100 pages; narrow the query or lower --limit");
        }
    }
    let summaries = runs.iter().map(Run::summary).collect::<Result<Vec<_>>>()?;
    args.output.print(&summaries, || {
        ui::heading("Workflow runs");
        ui::table(
            &["Run", "Status", "Event", "Ref", "Title"],
            &runs
                .iter()
                .map(|run| {
                    vec![
                        run.id.to_string(),
                        run.status.to_string(),
                        run.detail("event"),
                        run.detail("prettyref"),
                        run.detail("title"),
                    ]
                })
                .collect::<Vec<_>>(),
        );
    })
}

fn get(api: &Api, repo: &Repo, id: u64) -> Result<Run> {
    let id = id.to_string();
    let mut path = runs_path(repo);
    path.push(&id);
    let record: RunRecord = api.json(Method::GET, &path, &[], None)?;
    record.try_into()
}

fn get_jobs(api: &Api, repo: &Repo, id: u64) -> Result<Vec<Job>> {
    let id = id.to_string();
    let mut path = runs_path(repo);
    path.extend([&id, "jobs"]);
    api.json(Method::GET, &path, &[], None)
}

fn view(args: RunId, api: &Api, repo: &Repo) -> Result<()> {
    args.output.validate_fields(RUN_FIELDS)?;
    let run = get(api, repo, args.id)?;
    let value = serde_json::to_value(&run)?;
    args.output.print(&run, || {
        ui::heading(&format!("Workflow run {}", run.id));
        ui::table(
            &["Field", "Value"],
            &RUN_FIELDS
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

fn jobs(args: RunId, api: &Api, repo: &Repo) -> Result<()> {
    args.output.validate_fields(&[
        "id", "run_id", "name", "status", "attempt", "task_id", "needs", "runs_on",
    ])?;
    let jobs = get_jobs(api, repo, args.id)?;
    args.output.print(&jobs, || {
        ui::heading(&format!("Jobs for run {}", args.id));
        ui::table(
            &["Job", "Status", "Conclusion", "Runner"],
            &jobs
                .iter()
                .map(|job| {
                    vec![
                        job.name.clone(),
                        job.status.to_string(),
                        job.detail("conclusion"),
                        job.detail("runner_name"),
                    ]
                })
                .collect::<Vec<_>>(),
        );
    })
}

fn watch(args: Watch, api: &Api, repo: &Repo) -> Result<()> {
    args.output.validate_fields(RUN_FIELDS)?;
    let start = Instant::now();
    let timeout = Duration::from_secs(args.timeout);
    loop {
        let run = get(api, repo, args.id)?;
        ensure!(
            run.status.is_known(),
            "unsupported workflow run status: {}",
            run.status
        );
        if run.status.is_terminal() {
            args.output.print(&run, || {
                ui::heading(&format!("Workflow run {}", args.id));
                ui::status(
                    "Status",
                    &run.status.to_string(),
                    if run.status == Status::Success {
                        ui::Status::Success
                    } else {
                        ui::Status::Error
                    },
                );
            })?;
            ensure!(
                run.status == Status::Success,
                "workflow run {} finished with {}",
                args.id,
                run.status
            );
            return Ok(());
        }
        let remaining = timeout
            .checked_sub(start.elapsed())
            .context("timed out waiting for workflow run")?;
        eprintln!("Run {}: {}; waiting...", args.id, run.status);
        std::thread::sleep(Duration::from_secs(args.interval).min(remaining));
        ensure!(
            start.elapsed() < timeout,
            "timed out waiting for workflow run"
        );
    }
}

fn logs(args: Logs, api: &Api, repo: &Repo) -> Result<()> {
    args.output
        .validate_fields(&["id", "name", "status", "attempt", "log"])?;
    let mut logs = Vec::new();
    for job in get_jobs(api, repo, args.id)? {
        if args.failed && job.status != Status::Failure {
            continue;
        }
        let id = job.id.to_string();
        let mut path = repo.path().to_vec();
        path.extend(["actions", "jobs", &id, "logs"]);
        let attempt = job.attempt.flatten();
        let query = attempt
            .map(|attempt| vec![("attempt".into(), attempt.to_string())])
            .unwrap_or_default();
        let log = api.text(Method::GET, &path, &query, None)?;
        logs.push(JobLog {
            id: job.id,
            name: job.name,
            status: job.status,
            attempt,
            log,
        });
    }
    args.output.print(&logs, || {
        if logs.is_empty() {
            println!("No matching jobs.");
        }
        for job in &logs {
            ui::heading(&format!("{} ({})", job.name, job.status));
            ui::markdown(&job.log);
        }
    })
}

fn runs_path(repo: &Repo) -> Vec<&str> {
    let mut path = repo.path().to_vec();
    path.extend(["actions", "runs"]);
    path
}
