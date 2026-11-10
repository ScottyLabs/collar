use std::{
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use clap::{Args, Subcommand, ValueEnum};
use reqwest::Method;
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::{
    client::{Api, HttpStatus},
    output::Output,
    repo::Repo,
    ui,
};

const LABEL_LIMIT: usize = 1000;

#[derive(Subcommand)]
pub enum IssueCommand {
    /// List issues (pull requests are omitted).
    List {
        #[arg(long, value_enum, default_value = "open")]
        state: IssueState,
        #[arg(long, default_value_t = 30)]
        limit: usize,
        /// Only issues with this label; repeat to require every label.
        #[arg(long = "label", value_name = "NAME")]
        labels: Vec<String>,
        #[command(flatten)]
        output: Output,
    },
    /// View an issue by number.
    View(Selection),
    /// Create an issue.
    Create {
        #[arg(long)]
        title: String,
        #[command(flatten)]
        body: BodyInput,
        /// Add a repository or organization label by name; repeatable.
        #[arg(long = "label", value_name = "NAME")]
        labels: Vec<String>,
        #[command(flatten)]
        output: Output,
    },
    /// Edit an issue.
    Edit {
        number: String,
        #[arg(long)]
        title: Option<String>,
        #[command(flatten)]
        body: BodyInput,
        /// Add a repository or organization label by name; repeatable.
        #[arg(long = "add-label", value_name = "NAME")]
        add_labels: Vec<String>,
        /// Remove a label by name; repeatable.
        #[arg(long = "remove-label", value_name = "NAME")]
        remove_labels: Vec<String>,
        #[command(flatten)]
        output: Output,
    },
    /// Add a comment to an issue.
    Comment {
        number: String,
        #[command(flatten)]
        body: BodyInput,
        #[command(flatten)]
        output: Output,
    },
    /// Close an issue.
    Close(Selection),
    /// Reopen an issue.
    Reopen(Selection),
}

#[derive(Subcommand)]
pub enum LabelCommand {
    /// List repository labels.
    List {
        #[arg(long, default_value_t = 30)]
        limit: usize,
        #[command(flatten)]
        output: Output,
    },
    /// Create a repository label.
    Create {
        #[arg(long)]
        name: String,
        #[arg(long)]
        color: String,
        #[arg(long)]
        description: Option<String>,
        #[command(flatten)]
        output: Output,
    },
    /// Edit a repository label by ID.
    Edit {
        id: u64,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        color: Option<String>,
        #[arg(long)]
        description: Option<String>,
        #[command(flatten)]
        output: Output,
    },
    /// Delete a repository label by ID.
    Delete { id: u64 },
}

#[derive(Args)]
pub struct Selection {
    pub number: String,
    #[command(flatten)]
    pub output: Output,
}

#[derive(Args, Default)]
pub struct BodyInput {
    /// Exact body text.
    #[arg(long, conflicts_with = "body_file")]
    body: Option<String>,
    /// Read exact body text from a file, or stdin when set to -.
    #[arg(long, value_name = "PATH", conflicts_with = "body")]
    body_file: Option<PathBuf>,
}

impl BodyInput {
    fn read(&self) -> Result<Option<String>> {
        if let Some(body) = &self.body {
            return Ok(Some(body.clone()));
        }
        let Some(path) = &self.body_file else {
            return Ok(None);
        };
        let mut body = String::new();
        if path == Path::new("-") {
            io::stdin()
                .read_to_string(&mut body)
                .context("read body from stdin")?;
        } else {
            File::open(path)
                .with_context(|| format!("open body file {}", path.display()))?
                .read_to_string(&mut body)
                .with_context(|| format!("read body file {}", path.display()))?;
        }
        Ok(Some(body))
    }
}

#[derive(Clone, ValueEnum)]
pub enum IssueState {
    Open,
    Closed,
    All,
}

#[derive(Deserialize)]
pub struct User {
    login: String,
}

#[derive(Deserialize)]
pub struct Label {
    pub id: u64,
    pub name: String,
    pub color: String,
    pub description: Option<String>,
    #[serde(default)]
    pub exclusive: bool,
    #[serde(default)]
    pub is_archived: bool,
    pub url: Option<String>,
}

impl Label {
    const FIELDS: &'static [&'static str] = &[
        "id",
        "name",
        "color",
        "description",
        "exclusive",
        "isArchived",
        "url",
    ];

    fn output(&self) -> Value {
        json!({
            "id": self.id,
            "name": self.name,
            "color": self.color,
            "description": self.description,
            "exclusive": self.exclusive,
            "isArchived": self.is_archived,
            "url": self.url
        })
    }
}

#[derive(Deserialize)]
pub struct Issue {
    pub number: u64,
    pub title: String,
    pub body: Option<String>,
    pub state: String,
    pub html_url: String,
    pub user: User,
    pub labels: Vec<Label>,
    pub comments: u64,
    pub pull_request: Option<Value>,
}

impl Issue {
    const FIELDS: &'static [&'static str] = &[
        "number", "title", "body", "state", "url", "author", "labels", "comments",
    ];

    fn output(&self) -> Value {
        json!({
            "number": self.number,
            "title": self.title,
            "body": self.body.as_deref().unwrap_or(""),
            "state": self.state.to_uppercase(),
            "url": self.html_url,
            "author": {"login": self.user.login},
            "labels": self.labels.iter().map(Label::output).collect::<Vec<_>>(),
            "comments": self.comments
        })
    }
}

#[derive(Deserialize)]
struct Comment {
    id: u64,
    body: String,
    html_url: String,
    user: User,
}

impl Comment {
    fn output(&self) -> Value {
        json!({
            "id": self.id,
            "body": self.body,
            "url": self.html_url,
            "author": {"login": self.user.login}
        })
    }
}

pub fn run_issue(api: &Api, repo: &Repo, command: IssueCommand) -> Result<()> {
    match command {
        IssueCommand::List {
            state,
            limit,
            labels,
            output,
        } => list_issues(api, repo, state, limit, &labels, output),
        IssueCommand::View(selection) => view_issue(api, repo, selection),
        IssueCommand::Create {
            title,
            body,
            labels,
            output,
        } => create_issue(api, repo, title, body, &labels, output),
        IssueCommand::Edit {
            number: raw,
            title,
            body,
            add_labels,
            remove_labels,
            output,
        } => edit_issue(
            api,
            repo,
            raw,
            title,
            body,
            LabelChange {
                add: add_labels,
                remove: remove_labels,
            },
            output,
        ),
        IssueCommand::Comment {
            number: raw,
            body,
            output,
        } => comment_issue(api, repo, raw, body, output),
        IssueCommand::Close(selection) => set_state(api, repo, selection, "closed"),
        IssueCommand::Reopen(selection) => set_state(api, repo, selection, "open"),
    }
}

pub fn run_label(api: &Api, repo: &Repo, command: LabelCommand) -> Result<()> {
    match command {
        LabelCommand::List { limit, output } => list_labels(api, repo, limit, output),
        LabelCommand::Create {
            name,
            color,
            description,
            output,
        } => create_label(api, repo, name, color, description, output),
        LabelCommand::Edit {
            id,
            name,
            color,
            description,
            output,
        } => edit_label(api, repo, id, name, color, description, output),
        LabelCommand::Delete { id } => delete_label(api, repo, id),
    }
}

fn issue_path(repo: &Repo, number: u64) -> Vec<String> {
    let mut path: Vec<String> = repo.path().iter().map(|part| part.to_string()).collect();
    path.extend(["issues".into(), number.to_string()]);
    path
}

fn label_path(repo: &Repo, id: u64) -> Vec<String> {
    let mut path: Vec<String> = repo.path().iter().map(|part| part.to_string()).collect();
    path.extend(["labels".into(), id.to_string()]);
    path
}

fn path_refs(path: &[String]) -> Vec<&str> {
    path.iter().map(String::as_str).collect()
}

/// Labels usable on this repository's issues: its own, then its organization's.
fn available_labels(api: &Api, repo: &Repo) -> Result<Vec<Label>> {
    let mut path = repo.path().to_vec();
    path.push("labels");
    let mut labels: Vec<Label> = api.list(&path, &[], LABEL_LIMIT)?;
    match api.list::<Label>(&["orgs", &repo.owner, "labels"], &[], LABEL_LIMIT) {
        Ok(organization) => labels.extend(organization),
        // Repositories owned by users have no organization labels.
        Err(error) if HttpStatus::is(&error, 404) => {}
        Err(error) => return Err(error),
    }
    Ok(labels)
}

/// Resolve label names to IDs; repository labels shadow organization labels.
fn resolve_labels(available: &[Label], names: &[String]) -> Result<Vec<u64>> {
    names
        .iter()
        .map(|name| {
            ensure!(!name.trim().is_empty(), "label name must be nonempty");
            available
                .iter()
                .find(|label| label.name == *name)
                .map(|label| label.id)
                .with_context(|| {
                    let mut known: Vec<_> =
                        available.iter().map(|label| label.name.as_str()).collect();
                    known.sort_unstable();
                    known.dedup();
                    format!(
                        "unknown label {name:?}; available: {}",
                        if known.is_empty() {
                            "none".to_owned()
                        } else {
                            known.join(", ")
                        }
                    )
                })
        })
        .collect()
}

pub struct LabelChange {
    add: Vec<String>,
    remove: Vec<String>,
}

impl LabelChange {
    fn is_empty(&self) -> bool {
        self.add.is_empty() && self.remove.is_empty()
    }

    /// Reject contradictory requests before any network call.
    fn validate(&self) -> Result<()> {
        if let Some(name) = self.add.iter().find(|name| self.remove.contains(name)) {
            anyhow::bail!("label {name:?} is both added and removed");
        }
        Ok(())
    }

    /// The issue's label IDs after this change, or None when nothing changes.
    fn apply(&self, api: &Api, repo: &Repo, current: &[Label]) -> Result<Option<Vec<u64>>> {
        let available = available_labels(api, repo)?;
        let add = resolve_labels(&available, &self.add)?;
        for name in &self.remove {
            if !current.iter().any(|label| label.name == *name) {
                resolve_labels(&available, std::slice::from_ref(name))?;
            }
        }
        let before: Vec<u64> = current.iter().map(|label| label.id).collect();
        let mut after: Vec<u64> = current
            .iter()
            .filter(|label| !self.remove.contains(&label.name))
            .map(|label| label.id)
            .collect();
        for id in add {
            if !after.contains(&id) {
                after.push(id);
            }
        }
        Ok((after != before).then_some(after))
    }
}

fn get(api: &Api, repo: &Repo, number: u64) -> Result<Issue> {
    let path = issue_path(repo, number);
    let refs = path_refs(&path);
    api.json(Method::GET, &refs, &[], None)
}

fn ensure_issue(api: &Api, repo: &Repo, number: u64) -> Result<()> {
    let issue = get(api, repo, number)?;
    ensure!(
        issue.pull_request.is_none(),
        "number identifies a pull request, not an issue"
    );
    Ok(())
}

fn number(raw: &str) -> Result<u64> {
    let n: u64 = raw
        .parse()
        .context("issue number must be a positive integer")?;
    ensure!(n > 0, "issue number must be positive");
    Ok(n)
}

fn set_state(api: &Api, repo: &Repo, selection: Selection, state: &str) -> Result<()> {
    validate_write(api, &selection.output, Issue::FIELDS)?;
    let n = number(&selection.number)?;
    ensure_issue(api, repo, n)?;
    let path = issue_path(repo, n);
    let refs = path_refs(&path);
    let issue: Issue = api.json(
        Method::PATCH,
        &refs,
        &[],
        Some(&json!({
            "state": state
        })),
    )?;
    ensure!(
        issue.pull_request.is_none(),
        "Forgejo returned a pull request for issue state change"
    );
    selection
        .output
        .print(&issue.output(), || println!("{}", issue.html_url))
}

fn validate_label(name: &str, color: Option<&str>) -> Result<()> {
    ensure!(!name.trim().is_empty(), "label name must be nonempty");
    if let Some(color) = color {
        validate_color(color)?;
    }
    Ok(())
}

fn validate_color(color: &str) -> Result<()> {
    let color = color.strip_prefix('#').unwrap_or(color);
    ensure!(
        color.len() == 6 && color.bytes().all(|c| c.is_ascii_hexdigit()),
        "label color must be six hexadecimal digits, optionally prefixed with #"
    );
    Ok(())
}

fn list_issues(
    api: &Api,
    repo: &Repo,
    state: IssueState,
    limit: usize,
    labels: &[String],
    output: Output,
) -> Result<()> {
    ensure!(limit > 0, "list limit must be positive");
    output.validate_fields(Issue::FIELDS)?;
    let mut query = vec![
        (
            "state".into(),
            match state {
                IssueState::Open => "open",
                IssueState::Closed => "closed",
                IssueState::All => "all",
            }
            .into(),
        ),
        ("type".into(), "issues".into()),
    ];
    if !labels.is_empty() {
        ensure!(
            labels.iter().all(|name| !name.contains(',')),
            "label filters cannot contain commas"
        );
        // Forgejo ignores unknown names in this filter, which would list every issue.
        resolve_labels(&available_labels(api, repo)?, labels)?;
        query.push(("labels".into(), labels.join(",")));
    }
    let mut path = repo.path().to_vec();
    path.push("issues");
    let issues: Vec<Issue> = api.list(&path, &query, limit)?;
    ensure!(
        issues.iter().all(|issue| issue.pull_request.is_none()),
        "Forgejo returned a pull request for type=issues"
    );
    let values: Vec<_> = issues.iter().map(Issue::output).collect();
    output.print(&values, || {
        ui::heading("Issues");
        ui::table(
            &["Issue", "State", "Title", "Author", "Comments"],
            &issues
                .iter()
                .map(|issue| {
                    vec![
                        format!("#{}", issue.number),
                        issue.state.clone(),
                        issue.title.clone(),
                        issue.user.login.clone(),
                        issue.comments.to_string(),
                    ]
                })
                .collect::<Vec<_>>(),
        );
    })
}

fn view_issue(api: &Api, repo: &Repo, selection: Selection) -> Result<()> {
    selection.output.validate_fields(Issue::FIELDS)?;
    let issue = get(api, repo, number(&selection.number)?)?;
    ensure!(
        issue.pull_request.is_none(),
        "number identifies a pull request, not an issue"
    );
    selection.output.print(&issue.output(), || {
        ui::heading(&format!("Issue #{} {}", issue.number, issue.title));
        ui::detail("State", &issue.state);
        ui::detail("Author", &issue.user.login);
        ui::detail("URL", &issue.html_url);
        ui::detail("Comments", &issue.comments.to_string());
        if !issue.labels.is_empty() {
            ui::detail(
                "Labels",
                &issue
                    .labels
                    .iter()
                    .map(|label| label.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }
        if let Some(body) = issue.body.as_deref().filter(|body| !body.trim().is_empty()) {
            println!("\n{body}");
        }
    })
}

fn create_issue(
    api: &Api,
    repo: &Repo,
    title: String,
    body: BodyInput,
    labels: &[String],
    output: Output,
) -> Result<()> {
    validate_write(api, &output, Issue::FIELDS)?;
    ensure!(!title.trim().is_empty(), "title must be nonempty");
    let body = body.read()?.unwrap_or_default();
    let mut request = json!({
        "title": title,
        "body": body
    });
    if !labels.is_empty() {
        request["labels"] = resolve_labels(&available_labels(api, repo)?, labels)?.into();
    }
    let mut path = repo.path().to_vec();
    path.push("issues");
    let issue: Issue = api.json(Method::POST, &path, &[], Some(&request))?;
    ensure!(
        issue.pull_request.is_none(),
        "Forgejo returned a pull request for issue creation"
    );
    output.print(&issue.output(), || println!("{}", issue.html_url))
}

fn edit_issue(
    api: &Api,
    repo: &Repo,
    raw: String,
    title: Option<String>,
    body: BodyInput,
    labels: LabelChange,
    output: Output,
) -> Result<()> {
    validate_write(api, &output, Issue::FIELDS)?;
    let mut change = Map::new();
    if let Some(title) = title {
        ensure!(!title.trim().is_empty(), "title must be nonempty");
        change.insert("title".into(), title.into());
    }
    if let Some(body) = body.read()? {
        change.insert("body".into(), body.into());
    }
    ensure!(
        !change.is_empty() || !labels.is_empty(),
        "provide --title, --body/--body-file, --add-label, or --remove-label"
    );
    labels.validate()?;
    let number = number(&raw)?;
    let mut issue = get(api, repo, number)?;
    ensure!(
        issue.pull_request.is_none(),
        "number identifies a pull request, not an issue"
    );
    // Resolve every label before the first write.
    let label_ids = if labels.is_empty() {
        None
    } else {
        labels.apply(api, repo, &issue.labels)?
    };
    let path = issue_path(repo, number);
    let edited = !change.is_empty();
    if edited {
        issue = api.json(
            Method::PATCH,
            &path_refs(&path),
            &[],
            Some(&Value::Object(change)),
        )?;
        ensure!(
            issue.pull_request.is_none(),
            "Forgejo returned a pull request for issue edit"
        );
    }
    if let Some(ids) = label_ids {
        let mut path = path;
        path.push("labels".into());
        let result: Result<Vec<Label>> = api.json(
            Method::PUT,
            &path_refs(&path),
            &[],
            Some(&json!({ "labels": ids })),
        );
        issue.labels = if edited {
            result.context("the title or body was updated, but the labels were not")?
        } else {
            result?
        };
    }
    output.print(&issue.output(), || println!("{}", issue.html_url))
}

fn comment_issue(
    api: &Api,
    repo: &Repo,
    raw: String,
    body: BodyInput,
    output: Output,
) -> Result<()> {
    validate_write(api, &output, &["id", "body", "url", "author"])?;
    let body = body.read()?.context("provide --body or --body-file")?;
    ensure!(!body.trim().is_empty(), "comment body must be nonempty");
    let number = number(&raw)?;
    ensure_issue(api, repo, number)?;
    let number_text = number.to_string();
    let mut path = repo.path().to_vec();
    path.extend(["issues", &number_text, "comments"]);
    let comment: Comment = api.json(
        Method::POST,
        &path,
        &[],
        Some(&json!({
            "body": body
        })),
    )?;
    output.print(&comment.output(), || println!("{}", comment.html_url))
}

fn list_labels(api: &Api, repo: &Repo, limit: usize, output: Output) -> Result<()> {
    ensure!(limit > 0, "list limit must be positive");
    output.validate_fields(Label::FIELDS)?;
    let mut path = repo.path().to_vec();
    path.push("labels");
    let labels: Vec<Label> = api.list(&path, &[], limit)?;
    let values: Vec<_> = labels.iter().map(Label::output).collect();
    output.print(&values, || {
        ui::heading("Repository labels");
        ui::table(
            &["ID", "Name", "Color", "Description"],
            &labels
                .iter()
                .map(|label| {
                    vec![
                        label.id.to_string(),
                        label.name.clone(),
                        label.color.clone(),
                        label.description.clone().unwrap_or_default(),
                    ]
                })
                .collect::<Vec<_>>(),
        );
    })
}

fn create_label(
    api: &Api,
    repo: &Repo,
    name: String,
    color: String,
    description: Option<String>,
    output: Output,
) -> Result<()> {
    validate_write(api, &output, Label::FIELDS)?;
    validate_label(&name, Some(&color))?;
    let mut path = repo.path().to_vec();
    path.push("labels");
    let label: Label = api.json(
        Method::POST,
        &path,
        &[],
        Some(&json!({
            "name": name,
            "color": color,
            "description": description.unwrap_or_default()
        })),
    )?;
    output.print(&label.output(), || {
        ui::heading(&format!("Created label {}", label.name));
        ui::detail("Color", &label.color);
        if let Some(description) = &label.description {
            ui::detail("Description", description);
        }
    })
}

fn edit_label(
    api: &Api,
    repo: &Repo,
    id: u64,
    name: Option<String>,
    color: Option<String>,
    description: Option<String>,
    output: Output,
) -> Result<()> {
    validate_write(api, &output, Label::FIELDS)?;
    ensure!(id > 0, "label ID must be positive");
    let mut change = Map::new();
    if let Some(name) = name {
        validate_label(&name, None)?;
        change.insert("name".into(), name.into());
    }
    if let Some(color) = color {
        validate_color(&color)?;
        change.insert("color".into(), color.into());
    }
    if let Some(description) = description {
        change.insert("description".into(), description.into());
    }
    ensure!(
        !change.is_empty(),
        "provide --name, --color, or --description"
    );
    let path = label_path(repo, id);
    let refs = path_refs(&path);
    let label: Label = api.json(Method::PATCH, &refs, &[], Some(&Value::Object(change)))?;
    output.print(&label.output(), || {
        ui::heading(&format!("Updated label {}", label.name));
        ui::detail("Color", &label.color);
        if let Some(description) = &label.description {
            ui::detail("Description", description);
        }
    })
}

fn delete_label(api: &Api, repo: &Repo, id: u64) -> Result<()> {
    api.require_token()?;
    ensure!(id > 0, "label ID must be positive");
    let path = label_path(repo, id);
    api.text(Method::DELETE, &path_refs(&path), &[], None)?;
    ui::status("Label", &format!("Deleted {id}"), ui::Status::Success);
    Ok(())
}

fn validate_write(api: &Api, output: &Output, fields: &[&str]) -> Result<()> {
    output.validate_fields(fields)?;
    api.require_token()
}
