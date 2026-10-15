use anyhow::{Context, Result, bail, ensure};
use reqwest::Method;
use serde_json::{Value, json};

use super::commands::WorkflowCommand;
use crate::{client::Api, repo::Repo, ui};

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
                .context("validated workflow path has no filename")?;
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
