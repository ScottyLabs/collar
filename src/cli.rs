mod api;
mod auth;
mod completion;
mod infrastructure;
mod input;
mod kennel;
mod pr_args;
mod projects;
mod pull_requests;
mod repositories;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use self::api::ApiArgs;
use self::auth::AuthCommand;
use self::completion::CompletionShell;
use self::infrastructure::InfraCommand;
use self::kennel::KennelCommand;
use self::pr_args::PrCommand;
use self::projects::{ContextCommand, GovernanceCommand};
use self::repositories::RepoCommand;
use crate::auth::api;
use crate::config::Config;
use crate::source::Sources;
use crate::{changelog, config, doctor, environment, rfc};

#[derive(Parser)]
#[command(
    version,
    about = "ScottyLabs development workflows from your terminal",
    long_about = "Review Forgejo pull requests, inspect CI and previews, find project documentation, and run local development tools.",
    after_help = "Examples:\n  collar auth login\n  collar pr list -R ScottyLabs/collar\n  collar pr review 42 --approve\n  collar docs search secrets\n  collar check --all\n\nUse collar <command> --help for arguments and examples.\nDocumentation: https://docs.scottylabs.org/collar/"
)]
pub struct Cli {
    /// Forge origin; defaults to git.cmu.dev or the Git remote.
    #[arg(long, global = true, env = "COLLAR_HOST")]
    host: Option<String>,
    /// Select [HOST/][OWNER/]REPO; omitted owner defaults to ScottyLabs.
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
    /// Read and search canonical ScottyLabs documentation.
    #[command(
        after_long_help = "Examples:\n  collar docs search secrets\n  collar docs list --service kennel"
    )]
    Docs {
        #[command(subcommand)]
        command: crate::docs::Command,
    },
    /// Generate shell completions.
    #[command(
        after_long_help = "Examples:\n  collar completion bash\n  collar completion nushell\n\nSave or source the generated script using your shell's completion setup. Regenerate it after updating Collar."
    )]
    Completion {
        #[arg(value_enum)]
        shell: CompletionShell,
    },
    /// Work with Forgejo issues.
    #[command(
        after_long_help = "Examples:\n  collar issue list\n  collar issue create --title 'Preview times out' --body 'Steps to reproduce...'"
    )]
    Issue {
        #[command(subcommand)]
        command: crate::issues::IssueCommand,
    },
    /// Manage repository labels.
    #[command(
        after_long_help = "Examples:\n  collar label list\n  collar label create --name bug --color ff0000"
    )]
    Label {
        #[command(subcommand)]
        command: crate::issues::LabelCommand,
    },
    /// Inspect and manage repository releases.
    #[command(after_long_help = "Examples:\n  collar release list\n  collar release view v0.1.0")]
    Release {
        #[command(subcommand)]
        command: crate::releases::Command,
    },
    /// Inspect Forgejo Actions runs.
    #[command(after_long_help = "Examples:\n  collar actions list\n  collar actions jobs 123")]
    Actions {
        #[command(subcommand)]
        command: crate::actions::RunCommand,
    },
    /// Inspect or dispatch existing workflows.
    #[command(
        after_long_help = "Examples:\n  collar workflow dispatch ci.yml --ref main --field mode=fast"
    )]
    Workflow {
        #[command(subcommand)]
        command: crate::actions::WorkflowCommand,
    },
    /// Read ScottyLabs changelogs and tech-lead announcements.
    #[command(
        after_long_help = "Examples:\n  collar changelog view kennel\n  collar changelog updates"
    )]
    Changelog {
        #[command(subcommand)]
        command: changelog::Command,
    },
    /// Check or update project secrets through secretspec.
    #[command(
        after_long_help = "Examples:\n  collar secrets check --profile dev\n  collar secrets set JWT_SECRET --profile dev"
    )]
    Secrets {
        #[command(subcommand)]
        command: environment::SecretCommand,
    },
    /// Run project shells, processes, and tasks through devenv.
    #[command(
        after_long_help = "Examples:\n  collar dev shell\n  collar dev run -- cargo test\n  collar dev up"
    )]
    Dev {
        #[command(subcommand)]
        command: environment::DevCommand,
    },
    /// Run repository hooks inside the project's development shell.
    #[command(
        after_long_help = "Examples:\n  collar check\n  collar check --all\n  collar check treefmt --files src/main.rs\n\nRuns prek in the project development shell. No governance or secrets preflight is required; tool output and exit status are preserved."
    )]
    Check(environment::CheckArgs),
    /// Search RFCs or create a draft.
    Rfc(rfc::Args),
    /// Diagnose local setup and ScottyLabs login readiness.
    #[command(
        after_long_help = "Examples:\n  collar doctor\n  collar doctor --offline --json\n\nPrints the full setup report before returning failure. Offline mode skips network probes and reads cached sources. Secret values are never resolved."
    )]
    Doctor(doctor::Args),
    /// Connect the selected repository to its ScottyLabs project.
    #[command(
        after_long_help = "Examples:\n  collar project view -R ScottyLabs/collar\n  collar project select"
    )]
    Project {
        #[command(subcommand)]
        command: ContextCommand,
    },
    /// Manage nonsecret user defaults.
    #[command(
        after_long_help = "Examples:\n  collar config get\n  collar config set host git.cmu.dev"
    )]
    Config {
        #[command(subcommand)]
        command: config::Command,
    },
    /// Sign in to Forgejo or OpenBao and check saved credentials.
    #[command(
        after_long_help = "Examples:\n  collar auth login\n  collar auth status\n  collar auth bao login"
    )]
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
    /// Clone, inspect, or fork a repository.
    #[command(
        after_long_help = "Examples:\n  collar repo view -R ScottyLabs/collar\n  collar repo fork -R ScottyLabs/collar"
    )]
    Repo {
        #[command(subcommand)]
        command: RepoCommand,
    },
    /// Inspect and review pull requests.
    #[command(
        after_long_help = "Examples:\n  collar pr list\n  collar pr view 42\n  collar pr review 42 --approve"
    )]
    Pr {
        #[command(subcommand)]
        command: PrCommand,
    },
    /// Send a request to this forge's REST API.
    #[command(
        after_long_help = "Examples:\n  collar api user --method GET\n  collar api repos/ScottyLabs/collar/pulls\n\nEndpoints are relative to /api/v1 on the selected forge. GET is the default; fields imply POST unless --method is set. This command can write and requires the same authorization as the underlying operation."
    )]
    Api(ApiArgs),
    /// Discover declared ownership and service access without a clone.
    #[command(
        after_long_help = "Examples:\n  collar governance project list\n  collar governance project view collar"
    )]
    Governance {
        #[command(subcommand)]
        command: GovernanceCommand,
    },
    /// Inspect deployment reports from Forgejo.
    #[command(after_long_help = "Examples:\n  collar kennel preview 42\n  collar kennel wait 42")]
    Kennel {
        #[command(subcommand)]
        command: KennelCommand,
    },
    /// Inspect canonical infrastructure and build pinned Nix configurations.
    #[command(after_long_help = "Examples:\n  collar infra hosts\n  collar infra service kennel")]
    Infra {
        #[command(subcommand)]
        command: InfraCommand,
    },
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
        Command::Auth { command } => auth::run(command, &mut config, cli.host.as_deref()),
        Command::Changelog { command } => changelog::run(command, &sources),
        Command::Config { command } => config.run(command),
        Command::Secrets { command } => environment::secrets(command),
        Command::Dev { command } => environment::dev(command),
        Command::Check(args) => environment::checks(args),
        Command::Rfc(args) => rfc::run(args),
        Command::Doctor(args) => {
            doctor::run(args, &sources, cli.repo.as_deref(), cli.host.as_deref())
        }
        Command::Project { command } => projects::run_context(
            command,
            &sources,
            &config,
            cli.repo.as_deref(),
            cli.host.as_deref(),
        ),
        Command::Governance { command } => {
            projects::run_governance(command, &sources, cli.repo.as_deref(), cli.host.as_deref())
        }
        Command::Infra { command } => infrastructure::run(command, &sources),
        Command::Repo { command } => {
            repositories::run(command, &config, cli.repo.as_deref(), cli.host.as_deref())
        }
        Command::Api(args) => api::run(args, cli.host.as_deref(), &config),
        Command::Pr { command } => {
            let repo = config.repo(cli.repo.as_deref(), cli.host.as_deref())?;
            let api = api(repo.origin.clone(), &config)?;
            pull_requests::run(command, &api, &repo)
        }
        Command::Kennel { command } => {
            kennel::run(command, &config, cli.repo.as_deref(), cli.host.as_deref())
        }
    }
}

#[cfg(test)]
mod command_docs;
