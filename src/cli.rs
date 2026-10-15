use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use clap::{ArgGroup, Args, CommandFactory, Parser, Subcommand, ValueEnum};
use reqwest::Method;
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::auth::api;
use crate::client::{Api, host_url};
use crate::config::Config;
use crate::output::Output;
use crate::repo::Repo;
use crate::source::{Kind, Sources};
use crate::{
    changelog, checkout, config, doctor, environment, governance, governance_ops, infra, infra_ops,
    kennel, pr, project, rfc, tui, ui,
};

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

#[derive(Clone, Copy, ValueEnum)]
enum CompletionShell {
    Bash,
    Elvish,
    Fish,
    #[value(name = "powershell")]
    PowerShell,
    Zsh,
    Nushell,
}

impl CompletionShell {
    fn generate(self) {
        let mut command = Cli::command();
        let mut output = std::io::stdout();
        match self {
            Self::Nushell => clap_complete::generate(
                clap_complete_nushell::Nushell,
                &mut command,
                "collar",
                &mut output,
            ),
            shell => {
                let shell = match shell {
                    Self::Bash => clap_complete::Shell::Bash,
                    Self::Elvish => clap_complete::Shell::Elvish,
                    Self::Fish => clap_complete::Shell::Fish,
                    Self::PowerShell => clap_complete::Shell::PowerShell,
                    Self::Zsh => clap_complete::Shell::Zsh,
                    Self::Nushell => unreachable!(),
                };
                clap_complete::generate(shell, &mut command, "collar", &mut output);
            }
        }
    }
}

#[derive(Subcommand)]
enum Command {
    /// Read and search canonical ScottyLabs documentation.
    Docs {
        #[command(subcommand)]
        command: crate::docs::Command,
    },
    /// Generate shell completions.
    Completion {
        #[arg(value_enum)]
        shell: CompletionShell,
    },
    /// Work with Forgejo issues.
    Issue {
        #[command(subcommand)]
        command: crate::issues::IssueCommand,
    },
    /// Manage repository labels.
    Label {
        #[command(subcommand)]
        command: crate::issues::LabelCommand,
    },
    /// Inspect and manage repository releases.
    Release {
        #[command(subcommand)]
        command: crate::releases::Command,
    },
    /// Inspect Forgejo Actions runs.
    Actions {
        #[command(subcommand)]
        command: crate::actions::RunCommand,
    },
    /// Inspect or dispatch existing workflows.
    Workflow {
        #[command(subcommand)]
        command: crate::actions::WorkflowCommand,
    },
    /// Read ScottyLabs changelogs and tech-lead announcements.
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
    /// Verify and save a personal access token in the OS credential store.
    Login {
        /// Read the token from stdin instead of a hidden terminal prompt.
        #[arg(long)]
        with_token: bool,
        /// Use a hidden personal-access-token prompt instead of browser SSO.
        #[arg(long, conflicts_with_all = ["with_token", "web"])]
        token: bool,
        /// Explicitly launch browser SSO from this caller.
        #[arg(long, conflicts_with_all = ["with_token", "token"])]
        web: bool,
    },
    /// Remove this host's saved credential without revoking it on the server.
    Logout,
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
    /// Fork the selected repository without changing its settings.
    Fork {
        #[arg(long)]
        organization: Option<String>,
        #[arg(long)]
        name: Option<String>,
    },
    /// Clone using the repository's SSH URL and your existing Git authentication.
    Clone {
        repository: String,
        directory: Option<PathBuf>,
        /// Use HTTPS instead of the default SSH transport.
        #[arg(long)]
        https: bool,
    },
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
    /// Fetch a verified PR head and switch to a local branch.
    Checkout {
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
        selector: Option<String>,
        #[arg(long)]
        merge: bool,
        #[arg(long)]
        squash: bool,
        #[arg(long)]
        rebase: bool,
        /// Refuse if the current head differs from this commit.
        #[arg(long)]
        match_head_commit: Option<String>,
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
        selector: Option<String>,
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
        selector: Option<String>,
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
struct PullText {
    #[arg(short = 'b', long, conflicts_with = "body_file")]
    body: Option<String>,
    /// Read Markdown from a file, or - for stdin.
    #[arg(short = 'F', long)]
    body_file: Option<PathBuf>,
}

impl PullText {
    fn read(self) -> Result<Option<String>> {
        self.body_file
            .map(|path| read_file(&path))
            .transpose()
            .map(|file| file.or(self.body))
    }
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
    /// Add an inline comment; repeat for multiple comments. SIDE is old or new.
    #[arg(long = "inline-comment", num_args = 4, value_names = ["PATH", "LINE", "SIDE", "BODY"], action = clap::ArgAction::Append)]
    inline_comments: Vec<String>,
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
    #[command(flatten)]
    Workflows(governance_ops::Command),
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
    #[command(flatten)]
    Workflows(kennel::Command),
    /// Show build/deploy reports for a PR and historical preview comments.
    Status(Selection),
}

#[derive(Subcommand)]
enum InfraCommand {
    #[command(flatten)]
    Workflows(infra_ops::CommandKind),
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
        } | Command::Auth {
            command: AuthCommand::Login { .. }
        }
    );
    let mut config = Config::load(cli.config, cli.no_config, creating)?;
    let sources = Sources {
        config: &config,
        host: cli.host.as_deref(),
        cache: cli.cache_dir.as_deref(),
    };
    match cli.command {
        Command::Docs { command } => crate::docs::run(command, &sources),
        Command::Completion { shell } => {
            shell.generate();
            Ok(())
        }
        Command::Issue { command } => {
            let repo = config.repo(cli.repo.as_deref(), cli.host.as_deref())?;
            crate::issues::run_issue(&api(repo.origin.clone(), &config)?, &repo, command)
        }
        Command::Label { command } => {
            let repo = config.repo(cli.repo.as_deref(), cli.host.as_deref())?;
            crate::issues::run_label(&api(repo.origin.clone(), &config)?, &repo, command)
        }
        Command::Release { command } => {
            let repo = config.repo(cli.repo.as_deref(), cli.host.as_deref())?;
            crate::releases::run(command, &api(repo.origin.clone(), &config)?, &repo)
        }
        Command::Actions { command } => {
            let repo = config.repo(cli.repo.as_deref(), cli.host.as_deref())?;
            crate::actions::run_run(command, &api(repo.origin.clone(), &config)?, &repo)
        }
        Command::Workflow { command } => {
            let repo = config.repo(cli.repo.as_deref(), cli.host.as_deref())?;
            crate::actions::run_workflow(command, &api(repo.origin.clone(), &config)?, &repo)
        }
        Command::Auth {
            command:
                AuthCommand::Login {
                    with_token,
                    token,
                    web,
                },
        } => {
            let origin = host_url(cli.host.as_deref().unwrap_or(config.host()))?;
            crate::auth::login(&mut config, origin, with_token, token, web)
        }
        Command::Auth {
            command: AuthCommand::Logout,
        } => {
            let origin = host_url(cli.host.as_deref().unwrap_or(config.host()))?;
            crate::auth::logout(&mut config, origin)
        }
        Command::Changelog { command } => changelog::run(command, &sources),
        Command::Config { command } => config.run(command),
        Command::Secrets { command } => environment::secrets(command),
        Command::Dev { command } => environment::dev(command),
        Command::Check(args) => environment::checks(args),
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
                project.print();
                ui::detail("Source", &source.describe());
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
                project.print();
                ui::detail("Source", &source.describe());
            })
        }

        Command::Governance {
            command: GovernanceCommand::Workflows(command),
        } => governance_ops::run(command, &sources, cli.repo.as_deref(), cli.host.as_deref()),
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
                    ui::heading(&format!("Governance projects - {}", source.describe()));
                    ui::table(
                        &["Project", "Team", "Name"],
                        &projects
                            .iter()
                            .map(|project| {
                                vec![
                                    project.slug.clone(),
                                    project.team.clone(),
                                    project.name.clone(),
                                ]
                            })
                            .collect::<Vec<_>>(),
                    );
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
                    ui::heading(&format!("{} ({})", project.name, project.slug));
                    ui::detail("Team", &project.team);
                    ui::detail("Leads", &project.leads.join(", "));
                    ui::detail(
                        "Secret namespace",
                        project.secret_namespace.as_deref().unwrap_or(
                            "not defined for team-owned repos alongside nested projects",
                        ),
                    );
                    ui::table(
                        &["Repository", "Features"],
                        &project
                            .repos
                            .iter()
                            .map(|repo| {
                                vec![
                                    repo.name.clone(),
                                    repo.features.keys().cloned().collect::<Vec<_>>().join(", "),
                                ]
                            })
                            .collect::<Vec<_>>(),
                    );
                    ui::detail(
                        "Source",
                        &format!("{} - {}", project.source.display(), source.describe()),
                    );
                    ui::detail(
                        "Verification",
                        "Declared configuration; applied access is not verified.",
                    );
                })
            }
        },
        Command::Infra { command } => match command {
            InfraCommand::Workflows(command) => infra_ops::run(command, &sources),
            InfraCommand::Hosts {
                checkout,
                offline,
                output,
            } => {
                output.validate_fields(&["name", "configuration", "role", "dataSource"])?;
                let source = sources.load(Kind::Infrastructure, checkout, offline)?;
                let hosts = infra::hosts(&source.directory)?;
                output.print(&source.attach(&hosts)?, || {
                    ui::heading(&format!("Infrastructure hosts - {}", source.describe()));
                    ui::table(
                        &["Host", "Configuration"],
                        &hosts
                            .iter()
                            .map(|host| {
                                vec![host.name.clone(), host.configuration.display().to_string()]
                            })
                            .collect::<Vec<_>>(),
                    );
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
                    ui::heading(&format!("Infrastructure services - {}", source.describe()));
                    ui::table(
                        &["Host", "Service source"],
                        &services
                            .iter()
                            .map(|service| {
                                vec![service.host.clone(), service.source.display().to_string()]
                            })
                            .collect::<Vec<_>>(),
                    );
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
            command:
                RepoCommand::Clone {
                    repository,
                    directory,
                    https,
                },
        } => {
            let repo = config.repo(Some(&repository), cli.host.as_deref())?;
            let api = api(repo.origin.clone(), &config)?;
            crate::repo::clone(&api, &repo, directory.as_deref(), https)
        }
        Command::Repo {
            command: RepoCommand::Fork { organization, name },
        } => {
            let repo = config.repo(cli.repo.as_deref(), cli.host.as_deref())?;
            let api = api(repo.origin.clone(), &config)?;
            crate::repo::fork(&api, &repo, organization, name)
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
            command: KennelCommand::Workflows(command),
        } => {
            let repo = config.repo(cli.repo.as_deref(), cli.host.as_deref())?;
            let api = api(repo.origin.clone(), &config)?;
            kennel::run(command, &api, &repo)
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
            let comments: Vec<Comment> = api.json(Method::GET, &path, &[], None)?;
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
                ui::heading(&format!("Kennel reports for PR #{number}"));
                ui::detail("Commit", &pull.head.sha);
                ui::table(
                    &["Check", "State", "URL"],
                    &statuses
                        .iter()
                        .map(|status| {
                            vec![
                                status.context.clone(),
                                status.state.clone(),
                                status.target_url.clone().unwrap_or_default(),
                            ]
                        })
                        .collect::<Vec<_>>(),
                );
                for comment in &comments {
                    ui::detail("Preview", &comment.html_url);
                    println!("{}\n", comment.body);
                }
                ui::detail(
                    "Health",
                    "Forgejo reports only; live health is not verified.",
                );
            })
        }
    }
}

fn run_pr(command: PrCommand, api: &Api, repo: &Repo) -> Result<()> {
    match command {
        PrCommand::Checkout {
            selector,
            branch,
            remote,
        } => checkout::run(
            api,
            repo,
            selector.as_deref(),
            branch.as_deref(),
            remote.as_deref(),
        ),
        PrCommand::Merge {
            selector,
            merge: _,
            squash,
            rebase,
            match_head_commit,
            delete_branch,
            auto,
            output,
        } => {
            let method = if squash {
                "squash"
            } else if rebase {
                "rebase"
            } else {
                "merge"
            };
            output.validate_fields(&[
                "number",
                "headRefOid",
                "method",
                "autoMergeRequested",
                "accepted",
            ])?;
            api.require_token()?;
            let number = pr::select(api, repo, selector.as_deref())?;
            let pull = pr::open_head(api, repo, number, match_head_commit.as_deref())?;
            pr::merge(api, repo, &pull, method, delete_branch, auto)?;
            let value = json!({"number":number,"headRefOid":pull.head.sha,"method":method,"autoMergeRequested":auto,"accepted":true});
            output.print(&value, || {
                println!(
                    "Forgejo accepted the {method} request for PR #{number}; inspected head {}{}",
                    pull.head.sha,
                    if auto {
                        "; automatic merge requested without a pinned head"
                    } else {
                        ""
                    }
                )
            })
        }
        PrCommand::UpdateBranch {
            selector,
            rebase,
            match_head_commit,
            output,
        } => {
            output.validate_fields(&["number", "headRefOid", "method", "accepted"])?;
            api.require_token()?;
            let number = pr::select(api, repo, selector.as_deref())?;
            let pull = pr::open_head(api, repo, number, match_head_commit.as_deref())?;
            let style = if rebase { "rebase" } else { "merge" };
            pr::update_branch(api, repo, number, style)?;
            output.print(&json!({"number":number,"headRefOid":pull.head.sha,"method":style,"accepted":true}), || println!("Forgejo accepted the {style} branch update for PR #{number}; inspected head {}", pull.head.sha))
        }
        PrCommand::Create {
            title,
            head,
            base,
            text,
            output,
        } => {
            output.validate_fields(pr::Pull::FIELDS)?;
            api.require_token()?;
            let body = text.read()?.unwrap_or_default();
            let head = head.map(Ok).unwrap_or_else(crate::repo::branch)?;
            let pull = pr::create(api, repo, &title, &head, base.as_deref(), &body)?;
            output.print(&pull.output(), || println!("{}", pull.html_url))
        }
        PrCommand::Edit {
            selector,
            title,
            base,
            text,
            output,
        } => {
            output.validate_fields(pr::Pull::FIELDS)?;
            api.require_token()?;
            let body = text.read()?;
            let mut change = Map::new();
            for (key, value) in [("title", title), ("base", base), ("body", body)] {
                if let Some(value) = value {
                    if key != "body" {
                        ensure!(!value.trim().is_empty(), "{key} must be nonempty");
                    }
                    change.insert(key.into(), value.into());
                }
            }
            ensure!(
                !change.is_empty(),
                "provide --title, --base, --body, or --body-file"
            );
            let number = pr::select(api, repo, selector.as_deref())?;
            let pull = pr::edit(api, repo, number, &Value::Object(change))?;
            output.print(&pull.output(), || println!("{}", pull.html_url))
        }
        PrCommand::Comment {
            selector,
            text,
            output,
        } => {
            output.validate_fields(&["id", "body", "url", "author"])?;
            api.require_token()?;
            let body = text.read()?.context("provide --body or --body-file")?;
            ensure!(!body.trim().is_empty(), "comment body must be nonempty");
            let number = pr::select(api, repo, selector.as_deref())?;
            let comment = pr::comment(api, repo, number, &body)?;
            output.print(&comment.output(), || println!("{}", comment.html_url))
        }
        PrCommand::Close(selection) => set_pull_state(api, repo, selection, "closed"),
        PrCommand::Reopen(selection) => set_pull_state(api, repo, selection, "open"),
        PrCommand::Browse {
            state,
            limit,
            output,
        } => tui::browse_pull_requests(api, repo, state.as_str(), limit, &output),
        PrCommand::List {
            state,
            limit,
            author,
            base,
            head,
            output,
        } => {
            output.validate_fields(pr::Pull::FIELDS)?;
            let pulls = pr::filtered_list(
                api,
                repo,
                state.as_str(),
                limit,
                &pr::Filters { author, base, head },
            )?;
            let values: Vec<_> = pulls.iter().map(pr::Pull::output).collect();
            output.print(&values, || {
                ui::heading("Pull requests");
                ui::table(
                    &["PR", "State", "Title", "Author"],
                    &pulls
                        .iter()
                        .map(|pull| {
                            vec![
                                format!("#{}", pull.number),
                                if pull.merged {
                                    "merged".into()
                                } else {
                                    pull.state.clone()
                                },
                                pull.title.clone(),
                                pull.user.login.clone(),
                            ]
                        })
                        .collect::<Vec<_>>(),
                );
            })
        }
        PrCommand::View {
            selector,
            comments,
            output,
        } => {
            let mut fields = pr::Pull::FIELDS.to_vec();
            if comments {
                fields.extend(["comments", "reviews"]);
            }
            output.validate_fields(&fields)?;
            let number = pr::select(api, repo, selector.as_deref())?;
            let pull = pr::get(api, repo, number)?;
            let mut value = pull.output();
            if comments {
                pr::add_conversation(api, repo, number, &mut value)?;
            }
            output.print(&value, || {
                pull.print();
                if comments {
                    pr::print_conversation(&value);
                }
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
                ui::heading(&format!("Checks for PR #{}", pull.number));
                ui::table(
                    &["State", "Check", "Description", "URL"],
                    &statuses
                        .iter()
                        .map(|status| {
                            vec![
                                status.state.clone(),
                                status.context.clone(),
                                status.description.clone().unwrap_or_default(),
                                status.target_url.clone().unwrap_or_default(),
                            ]
                        })
                        .collect::<Vec<_>>(),
                );
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
            let id = review.get("id").and_then(Value::as_u64).context(
                "review response has no ID; verify the review in the forge before retrying",
            )?;
            println!("Submitted {event} review {id} on PR #{number}");
            Ok(())
        }
    }
}

fn set_pull_state(api: &Api, repo: &Repo, selection: Selection, state: &str) -> Result<()> {
    selection.output.validate_fields(pr::Pull::FIELDS)?;
    api.require_token()?;
    let number = pr::select(api, repo, selection.selector.as_deref())?;
    let current = pr::get(api, repo, number)?;
    ensure!(!current.merged, "a merged PR cannot be closed or reopened");
    ensure!(current.state != state, "PR is already {state}");
    let pull = pr::edit(api, repo, number, &json!({"state":state}))?;
    selection
        .output
        .print(&pull.output(), || println!("{}", pull.html_url))
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
