use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use clap::{ArgGroup, Args, Parser, Subcommand, ValueEnum};
use reqwest::Method;
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::auth::api;
use crate::client::{Api, host_url};
use crate::config::Config;
use crate::output::Output;
use crate::repo::Repo;
use crate::source::{Kind, Sources};
use crate::{changelog, config, doctor, environment, governance, infra, pr, project, rfc, tui};

#[derive(Parser)]
#[command(version, about = "ScottyLabs forge, governance, and deployment tools")]
pub struct Cli {
    /// Forge origin; defaults to git.cmu.dev or the Git remote.
    #[arg(long, global = true, env = "COLLAR_HOST")]
    host: Option<String>,
    /// Select [HOST/]OWNER/REPO instead of the Git remote.
    #[arg(short = 'R', long, global = true)]
    repo: Option<String>,
    /// Use a specific nonsecret settings file.
    #[arg(
        long,
        global = true,
        env = "COLLAR_CONFIG",
        conflicts_with = "no_config"
    )]
    config: Option<PathBuf>,
    /// Override the user cache directory for canonical source snapshots.
    #[arg(long, global = true, env = "COLLAR_CACHE_DIR")]
    cache_dir: Option<PathBuf>,
    /// Ignore saved settings for reproducible automation.
    #[arg(long, global = true)]
    no_config: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Read ScottyLabs changelogs and prepare tech-lead announcements.
    Changelog {
        #[command(subcommand)]
        command: changelog::Command,
    },
    /// Check or update project secrets through secretspec.
    Secrets {
        #[command(subcommand)]
        command: environment::SecretCommand,
    },
    /// Run project shells, processes, and tasks through devenv.
    Dev {
        #[command(subcommand)]
        command: environment::DevCommand,
    },
    /// Run repository hooks inside the project's development shell.
    Check(environment::CheckArgs),
    /// Search RFCs or create a draft.
    Rfc(rfc::Args),
    /// Diagnose local setup and ScottyLabs login readiness.
    Doctor(doctor::Args),
    /// Connect the selected repository to its ScottyLabs project.
    Project {
        #[command(subcommand)]
        command: ContextCommand,
    },
    /// Manage nonsecret user defaults.
    Config {
        #[command(subcommand)]
        command: config::Command,
    },
    /// Inspect authentication.
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
    /// Inspect repositories.
    Repo {
        #[command(subcommand)]
        command: RepoCommand,
    },
    /// Inspect and review pull requests.
    Pr {
        #[command(subcommand)]
        command: PrCommand,
    },
    /// Send a request to this forge's REST API.
    Api(ApiArgs),
    /// Inspect declared project ownership and features.
    Governance {
        #[command(subcommand)]
        command: GovernanceCommand,
    },
    /// Inspect deployment reports from Forgejo.
    Kennel {
        #[command(subcommand)]
        command: KennelCommand,
    },
    /// Find infrastructure configuration in a local checkout.
    Infra {
        #[command(subcommand)]
        command: InfraCommand,
    },
}

#[derive(Subcommand)]
enum ContextCommand {
    /// Pick a project repository interactively; emits its context.
    Select {
        #[arg(long, env = "COLLAR_GOVERNANCE")]
        checkout: Option<PathBuf>,
        #[arg(long)]
        offline: bool,
        #[command(flatten)]
        output: Output,
    },
    /// Show declared ownership, features, and the secret namespace.
    View {
        #[arg(long, env = "COLLAR_GOVERNANCE")]
        checkout: Option<PathBuf>,
        /// Read an existing snapshot without refreshing it.
        #[arg(long)]
        offline: bool,
        #[command(flatten)]
        output: Output,
    },
}

#[derive(Subcommand)]
enum AuthCommand {
    /// Authenticate with ScottyLabs OpenBao.
    Bao {
        #[command(subcommand)]
        command: environment::BaoCommand,
    },
    /// Verify the configured token against the forge.
    Status,
}

#[derive(Subcommand)]
enum RepoCommand {
    /// View a repository.
    View {
        /// Repository as [HOST/]OWNER/REPO.
        repository: Option<String>,
        #[command(flatten)]
        output: Output,
    },
}

#[derive(Subcommand)]
enum PrCommand {
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
        #[command(flatten)]
        output: Output,
    },
    /// View a PR selected by number, URL, or branch (default: current branch).
    View(Selection),
    /// Print a PR's unified diff.
    Diff { selector: Option<String> },
    /// Show commit statuses; fail when none are reported or any are not successful.
    Checks(Selection),
    /// Submit a review, optionally including inline comments.
    Review(ReviewArgs),
}

#[derive(Clone, ValueEnum)]
enum State {
    Open,
    Closed,
    All,
}

impl State {
    fn as_str(&self) -> &str {
        match self {
            Self::Open => "open",
            Self::Closed => "closed",
            Self::All => "all",
        }
    }
}

#[derive(Args)]
struct Selection {
    selector: Option<String>,
    #[command(flatten)]
    output: Output,
}

#[derive(Args)]
#[command(group(ArgGroup::new("verdict").required(true).args(["approve", "request_changes", "comment"])))]
struct ReviewArgs {
    selector: Option<String>,
    #[arg(short = 'a', long)]
    approve: bool,
    #[arg(short = 'r', long)]
    request_changes: bool,
    #[arg(short = 'c', long)]
    comment: bool,
    #[arg(short = 'b', long, conflicts_with = "body_file")]
    body: Option<String>,
    /// Read review text from a file, or - for stdin.
    #[arg(short = 'F', long)]
    body_file: Option<PathBuf>,
    /// JSON array of {path,line,side,body}; side is old or new.
    #[arg(long)]
    comments_file: Option<PathBuf>,
}

#[derive(Args)]
struct ApiArgs {
    endpoint: String,
    #[arg(short = 'X', long)]
    method: Option<String>,
    /// Add a string field; fields imply POST unless --method is given.
    #[arg(short = 'f', long = "raw-field")]
    raw_fields: Vec<String>,
    /// Add a typed JSON field; @file and @- read a string from a file or stdin.
    #[arg(short = 'F', long = "field")]
    fields: Vec<String>,
    /// Read a JSON request body from a file, or - for stdin.
    #[arg(long, conflicts_with_all = ["raw_fields", "fields"])]
    input: Option<PathBuf>,
}

#[derive(Subcommand)]
enum GovernanceCommand {
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
}

#[derive(Subcommand)]
enum ProjectCommand {
    List {
        #[arg(long, env = "COLLAR_GOVERNANCE")]
        checkout: Option<PathBuf>,
        /// Read an existing snapshot without refreshing it.
        #[arg(long)]
        offline: bool,
        #[command(flatten)]
        output: Output,
    },
    View {
        /// Project slug or repository name.
        name: String,
        #[arg(long, env = "COLLAR_GOVERNANCE")]
        checkout: Option<PathBuf>,
        /// Read an existing snapshot without refreshing it.
        #[arg(long)]
        offline: bool,
        #[command(flatten)]
        output: Output,
    },
}

#[derive(Subcommand)]
enum KennelCommand {
    /// Show build/deploy reports for a PR and historical preview comments.
    Status(Selection),
}

#[derive(Subcommand)]
enum InfraCommand {
    Hosts {
        #[arg(long, env = "COLLAR_INFRASTRUCTURE")]
        checkout: Option<PathBuf>,
        /// Read an existing snapshot without refreshing it.
        #[arg(long)]
        offline: bool,
        #[command(flatten)]
        output: Output,
    },
    Service {
        name: String,
        #[arg(long, env = "COLLAR_INFRASTRUCTURE")]
        checkout: Option<PathBuf>,
        /// Read an existing snapshot without refreshing it.
        #[arg(long)]
        offline: bool,
        #[command(flatten)]
        output: Output,
    },
}

fn read_file(path: &Path) -> Result<String> {
    if path == Path::new("-") {
        let mut text = String::new();
        std::io::stdin()
            .read_to_string(&mut text)
            .context("read stdin")?;
        Ok(text)
    } else {
        std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))
    }
}

pub fn run(cli: Cli) -> Result<()> {
    let creating = matches!(
        &cli.command,
        Command::Config {
            command: config::Command::Set { .. }
        }
    );
    let config = Config::load(cli.config, cli.no_config, creating)?;
    let sources = Sources {
        config: &config,
        host: cli.host.as_deref(),
        cache: cli.cache_dir.as_deref(),
    };
    match cli.command {
        Command::Changelog { command } => changelog::run(command, &sources),
        Command::Config { command } => config.run(command),
        Command::Secrets { command } => {
            environment::secrets(command, &sources, cli.repo.as_deref(), cli.host.as_deref())
        }
        Command::Dev { command } => {
            environment::dev(command, &sources, cli.repo.as_deref(), cli.host.as_deref())
        }
        Command::Check(args) => {
            environment::checks(args, &sources, cli.repo.as_deref(), cli.host.as_deref())
        }
        Command::Auth {
            command: AuthCommand::Bao { command },
        } => environment::bao(command),
        Command::Rfc(args) => rfc::run(args),
        Command::Doctor(args) => {
            doctor::run(args, &sources, cli.repo.as_deref(), cli.host.as_deref())
        }
        Command::Project {
            command:
                ContextCommand::Select {
                    checkout,
                    offline,
                    output,
                },
        } => {
            tui::require_terminal()?;
            let mut fields = project::Project::FIELDS.to_vec();
            fields.push("dataSource");
            output.validate_fields(&fields)?;
            let source = sources.load(Kind::Governance, checkout, offline)?;
            let projects = governance::projects(&source.directory)?;
            let choices: Vec<_> = projects
                .iter()
                .flat_map(|project| project.repos.iter().map(move |repo| (project, repo)))
                .collect();
            let labels: Vec<_> = choices
                .iter()
                .map(|(project, repo)| format!("{}/{} - {}", project.team, project.slug, repo.name))
                .collect();
            let (_, selected) = choices[tui::choose("Project", &labels)?];
            let repo = project::repository(&source.directory, &selected.name)?;
            if let Some(host) = cli.host.as_deref() {
                ensure!(
                    repo.origin == host_url(host)?,
                    "governance repository host disagrees with --host"
                );
            }
            let project = project::resolve(&source.directory, &repo)?;
            output.print(&source.attach(&project)?, || {
                println!("{}", source.describe());
                project.print();
            })
        }
        Command::Project {
            command:
                ContextCommand::View {
                    checkout,
                    offline,
                    output,
                },
        } => {
            let mut fields = project::Project::FIELDS.to_vec();
            fields.push("dataSource");
            output.validate_fields(&fields)?;
            let repo = config.repo(cli.repo.as_deref(), cli.host.as_deref())?;
            let source = sources.load_at(Kind::Governance, checkout, offline, &repo.origin)?;
            let project = project::resolve(&source.directory, &repo)?;
            output.print(&source.attach(&project)?, || {
                println!("{}", source.describe());
                project.print();
            })
        }

        Command::Governance {
            command: GovernanceCommand::Project { command },
        } => match command {
            ProjectCommand::List {
                checkout,
                offline,
                output,
            } => {
                output.validate_fields(&[
                    "slug",
                    "name",
                    "team",
                    "leads",
                    "members",
                    "repos",
                    "secretNamespace",
                    "source",
                    "dataSource",
                ])?;
                let source = sources.load(Kind::Governance, checkout, offline)?;
                let projects = governance::projects(&source.directory)?;
                output.print(&source.attach(&projects)?, || {
                    println!("{}", source.describe());
                    for project in &projects {
                        println!("{}\t{}\t{}", project.slug, project.team, project.name);
                    }
                })
            }
            ProjectCommand::View {
                name,
                checkout,
                offline,
                output,
            } => {
                let source = sources.load(Kind::Governance, checkout, offline)?;
                let projects = governance::projects(&source.directory)?;
                let matches: Vec<_> = projects
                    .iter()
                    .filter(|project| {
                        project.slug == name || project.repos.iter().any(|repo| repo.name == name)
                    })
                    .collect();
                ensure!(
                    matches.len() == 1,
                    "expected one governance project for {name}, found {}",
                    matches.len()
                );
                let project = matches[0];
                output.print(&source.attach(project)?, || {
                    println!("{}", source.describe());
                    println!(
                        "{} ({})\nTeam: {}\nLeads: {}\nSecret namespace: {}",
                        project.name,
                        project.slug,
                        project.team,
                        project.leads.join(", "),
                        project.secret_namespace.as_deref().unwrap_or(
                            "not defined for team-owned repos alongside nested projects"
                        )
                    );
                    for repo in &project.repos {
                        println!(
                            "{}: {}",
                            repo.name,
                            repo.features.keys().cloned().collect::<Vec<_>>().join(", ")
                        );
                    }
                    println!(
                        "Source: {}\nDeclared configuration; applied access is not verified.",
                        project.source.display()
                    );
                })
            }
        },
        Command::Infra { command } => match command {
            InfraCommand::Hosts {
                checkout,
                offline,
                output,
            } => {
                output.validate_fields(&["name", "configuration", "role", "dataSource"])?;
                let source = sources.load(Kind::Infrastructure, checkout, offline)?;
                let hosts = infra::hosts(&source.directory)?;
                output.print(&source.attach(&hosts)?, || {
                    println!("{}", source.describe());
                    for host in &hosts {
                        println!("{}\t{}", host.name, host.configuration.display());
                    }
                })
            }
            InfraCommand::Service {
                name,
                checkout,
                offline,
                output,
            } => {
                output.validate_fields(&["host", "source", "dataSource"])?;
                let source = sources.load(Kind::Infrastructure, checkout, offline)?;
                let services = infra::service(&source.directory, &name)?;
                output.print(&source.attach(&services)?, || {
                    println!("{}", source.describe());
                    for service in &services {
                        println!("{}\t{}", service.host, service.source.display());
                    }
                })
            }
        },
        Command::Auth {
            command: AuthCommand::Status,
        } => {
            let api = api(
                host_url(cli.host.as_deref().unwrap_or(config.host()))?,
                &config,
            )?;
            api.require_token()?;
            let user: pr::User = api.json(Method::GET, &["user"], &[], None)?;
            println!("Logged in to {} as {}", api.origin(), user.login);
            Ok(())
        }
        Command::Repo {
            command: RepoCommand::View { repository, output },
        } => {
            let repo = config.repo(
                repository.as_deref().or(cli.repo.as_deref()),
                cli.host.as_deref(),
            )?;
            let api = api(repo.origin.clone(), &config)?;
            #[derive(Deserialize)]
            struct Repository {
                full_name: String,
                description: Option<String>,
                html_url: String,
                default_branch: String,
                private: bool,
                archived: bool,
            }
            let repository: Repository = api.json(Method::GET, &repo.path(), &[], None)?;
            let value = json!({ "nameWithOwner": repository.full_name, "description": repository.description, "url": repository.html_url, "defaultBranchRef": { "name": repository.default_branch }, "isPrivate": repository.private, "isArchived": repository.archived });
            output.print(&value, || {
                println!(
                    "{}\n{}\n{}",
                    repository.full_name,
                    repository.description.as_deref().unwrap_or(""),
                    repository.html_url
                )
            })
        }
        Command::Api(args) => raw_api(args, cli.host.as_deref(), &config),
        Command::Pr { command } => {
            let repo = config.repo(cli.repo.as_deref(), cli.host.as_deref())?;
            let api = api(repo.origin.clone(), &config)?;
            run_pr(command, &api, &repo)
        }
        Command::Kennel {
            command: KennelCommand::Status(selection),
        } => {
            let repo = config.repo(cli.repo.as_deref(), cli.host.as_deref())?;
            let api = api(repo.origin.clone(), &config)?;
            let number = pr::select(&api, &repo, selection.selector.as_deref())?;
            let pull = pr::get(&api, &repo, number)?;
            let statuses: Vec<_> = pr::statuses(&api, &repo, &pull.head.sha)?
                .into_iter()
                .filter(|status| {
                    matches!(status.context.as_str(), "kennel/build" | "kennel/deploy")
                })
                .collect();
            #[derive(Deserialize, serde::Serialize)]
            struct Comment {
                body: String,
                html_url: String,
            }
            let mut path = repo.path().to_vec();
            let number_string = number.to_string();
            path.extend(["issues", &number_string, "comments"]);
            let comments: Vec<Comment> = api.list(&path, &[], usize::MAX)?;
            let comments: Vec<_> = comments
                .into_iter()
                .filter(|comment| comment.body.contains("<!-- kennel-deployment -->"))
                .collect();
            ensure!(
                !statuses.is_empty() || !comments.is_empty(),
                "no Kennel reports found for PR #{number}; check governance registration and the webhook"
            );
            let value = json!({ "number": number, "headRefOid": pull.head.sha, "statuses": statuses, "comments": comments, "liveHealthVerified": false });
            selection.output.print(&value, || {
                println!("PR #{number}, commit {}", pull.head.sha);
                for status in &statuses { println!("{}\t{}\t{}", status.context, status.state, status.target_url.as_deref().unwrap_or("")); }
                for comment in &comments { println!("{}\n{}", comment.html_url, comment.body); }
                println!("Forgejo reports only; preview comments may be historical. Live health is not verified.");
            })
        }
    }
}

fn run_pr(command: PrCommand, api: &Api, repo: &Repo) -> Result<()> {
    match command {
        PrCommand::Browse {
            state,
            limit,
            output,
        } => tui::browse_pull_requests(api, repo, state.as_str(), limit, &output),
        PrCommand::List {
            state,
            limit,
            output,
        } => {
            output.validate_fields(pr::Pull::FIELDS)?;
            let pulls = pr::list(api, repo, state.as_str(), limit)?;
            let values: Vec<_> = pulls.iter().map(pr::Pull::output).collect();
            output.print(&values, || {
                for pull in &pulls {
                    println!(
                        "#{}\t{}\t{}\t{}",
                        pull.number, pull.state, pull.title, pull.user.login
                    );
                }
            })
        }
        PrCommand::View(selection) => {
            let number = pr::select(api, repo, selection.selector.as_deref())?;
            let pull = pr::get(api, repo, number)?;
            selection.output.print(&pull.output(), || {
                println!(
                    "#{} {}\n{} -> {}\n{}\n\n{}",
                    pull.number,
                    pull.title,
                    pull.head.name,
                    pull.base.name,
                    pull.html_url,
                    pull.body.as_deref().unwrap_or("")
                )
            })
        }
        PrCommand::Diff { selector } => {
            let number = pr::select(api, repo, selector.as_deref())?;
            print!("{}", pr::diff(api, repo, number)?);
            Ok(())
        }
        PrCommand::Checks(selection) => {
            selection
                .output
                .validate_fields(&["name", "state", "description", "link"])?;
            let number = pr::select(api, repo, selection.selector.as_deref())?;
            let pull = pr::get(api, repo, number)?;
            let statuses = pr::statuses(api, repo, &pull.head.sha)?;
            let values: Vec<_> = statuses.iter().map(|status| json!({ "name": status.context, "state": status.state, "description": status.description, "link": status.target_url })).collect();
            selection.output.print(&values, || {
                for status in &statuses {
                    println!(
                        "{}\t{}\t{}",
                        status.state,
                        status.context,
                        status.target_url.as_deref().unwrap_or("")
                    );
                }
            })?;
            ensure!(
                !statuses.is_empty(),
                "no commit statuses reported; required checks are not verified"
            );
            ensure!(
                statuses.iter().all(|status| status.state == "success"),
                "one or more reported commit statuses are not successful"
            );
            Ok(())
        }
        PrCommand::Review(args) => {
            api.require_token()?;
            let number = pr::select(api, repo, args.selector.as_deref())?;
            ensure!(
                !(args.body_file.as_deref() == Some(Path::new("-"))
                    && args.comments_file.as_deref() == Some(Path::new("-"))),
                "review body and inline comments cannot both read stdin"
            );
            let body = match args.body_file {
                Some(path) => read_file(&path)?,
                None => args.body.unwrap_or_default(),
            };
            let comments: Vec<pr::LineComment> = args
                .comments_file
                .map(|path| {
                    read_file(&path).and_then(|text| {
                        serde_json::from_str(&text).context("parse inline comments JSON")
                    })
                })
                .transpose()?
                .unwrap_or_default();
            let event = if args.approve {
                "APPROVED"
            } else if args.request_changes {
                "REQUEST_CHANGES"
            } else {
                "COMMENT"
            };
            let review = pr::review(api, repo, number, event, &body, &comments)?;
            let id = review.get("id").and_then(Value::as_u64).context(
                "review response has no ID; verify the review in the forge before retrying",
            )?;
            println!("Submitted {event} review {id} on PR #{number}");
            Ok(())
        }
    }
}

fn raw_api(args: ApiArgs, host: Option<&str>, config: &Config) -> Result<()> {
    let api = api(host_url(host.unwrap_or(config.host()))?, config)?;
    ensure!(
        !args.endpoint.contains([':', '?', '#', '%']),
        "API endpoint must be a relative path without a query or URL escapes; use -f/-F for fields"
    );
    let path: Vec<_> = args.endpoint.trim_start_matches('/').split('/').collect();
    let mut fields = Map::new();
    for (typed, entries) in [(false, args.raw_fields), (true, args.fields)] {
        for field in entries {
            let (name, value) = field
                .split_once('=')
                .context("API fields must be KEY=VALUE")?;
            ensure!(
                !name.is_empty() && !name.contains(['[', ']']),
                "nested or empty API field names are not supported"
            );
            let value = if typed {
                if let Some(path) = value.strip_prefix('@') {
                    Value::String(read_file(Path::new(path))?)
                } else {
                    serde_json::from_str(value).unwrap_or_else(|_| Value::String(value.into()))
                }
            } else {
                Value::String(value.into())
            };
            ensure!(
                fields.insert(name.into(), value).is_none(),
                "duplicate API field: {name}"
            );
        }
    }
    let method = match args.method {
        Some(method) => {
            Method::from_bytes(method.to_uppercase().as_bytes()).context("invalid HTTP method")?
        }
        None if !fields.is_empty() || args.input.is_some() => Method::POST,
        None => Method::GET,
    };
    let mut query = Vec::new();
    let body = if matches!(method, Method::GET | Method::HEAD) {
        ensure!(args.input.is_none(), "GET/HEAD cannot use --input");
        for (key, value) in fields {
            query.push((
                key,
                match value {
                    Value::String(text) => text,
                    value => value.to_string(),
                },
            ));
        }
        None
    } else if let Some(path) = args.input {
        Some(serde_json::from_str(&read_file(&path)?).context("parse API request JSON")?)
    } else if fields.is_empty() {
        None
    } else {
        Some(Value::Object(fields))
    };
    let text = api.text(method, &path, &query, body.as_ref())?;
    if !text.is_empty() {
        let value: Value = serde_json::from_str(&text).context("API response is not JSON")?;
        println!("{}", serde_json::to_string_pretty(&value)?);
    }
    Ok(())
}
