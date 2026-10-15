use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use anyhow::{Context, Result, ensure};
use clap::{Args, Subcommand};
use serde::Deserialize;

use crate::{process, repo};

pub const BAO_ADDR: &str = "https://secrets.scottylabs.org";

#[derive(Subcommand)]
pub enum BaoCommand {
    /// Authenticate with ScottyLabs OIDC before entering a project shell.
    Login,
    /// Verify the current token without displaying it.
    Status,
}

#[derive(Args)]
pub struct ContextArgs {
    /// Secret profile; uses the underlying tool default when omitted.
    #[arg(long, env = "SECRETSPEC_PROFILE")]
    profile: Option<String>,
    /// Audit reason, forwarded unchanged to secretspec and devenv.
    #[arg(long, env = "SECRETSPEC_REASON")]
    reason: Option<String>,
    /// Select a provider alias declared in secretspec.toml.
    #[arg(long, env = "SECRETSPEC_PROVIDER")]
    provider: Option<String>,
}

#[derive(Subcommand)]
pub enum SecretCommand {
    /// Check required secrets without prompts or printing values.
    Check {
        #[command(flatten)]
        context: ContextArgs,
        #[arg(long, env = "SECRETSPEC_SCOPE")]
        scope: Option<String>,
        /// Emit secretspec's value-free JSON report.
        #[arg(long)]
        json: bool,
    },
    /// Set a declared secret using a prompt or file/stdin input.
    Set {
        name: String,
        /// Read exact bytes from a file, or - for stdin; no value arguments.
        #[arg(long)]
        from_file: Option<PathBuf>,
        #[command(flatten)]
        context: ContextArgs,
    },
}

#[derive(Subcommand)]
pub enum DevCommand {
    /// Enter an interactive project shell.
    Shell(ContextArgs),
    /// Run a command inside the project shell, preserving literal arguments.
    Run {
        #[command(flatten)]
        context: ContextArgs,
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        command: Vec<OsString>,
    },
    /// Start the project's configured processes.
    Up {
        /// Start configured processes in the background.
        #[arg(short = 'd', long)]
        detach: bool,
        #[command(flatten)]
        context: ContextArgs,
    },
    /// Stop local processes without resolving project ownership or secrets.
    Down {
        #[arg(long, env = "SECRETSPEC_REASON")]
        reason: Option<String>,
    },
    /// Run a named devenv task or namespace.
    Task {
        name: String,
        #[command(flatten)]
        context: ContextArgs,
    },
}

#[derive(Args)]
pub struct CheckArgs {
    /// Run only this hook.
    hook: Option<String>,
    /// Run hooks for all files instead of only staged files.
    #[arg(long, conflicts_with = "files")]
    all: bool,
    /// Existing files relative to the calling directory, or absolute paths.
    #[arg(long, num_args = 1..)]
    files: Vec<PathBuf>,
    #[command(flatten)]
    context: ContextArgs,
}

#[derive(Deserialize)]
pub struct Secretspec {
    project: SecretProject,
    profiles: BTreeMap<String, BTreeMap<String, toml::Value>>,
}

#[derive(Deserialize)]
struct SecretProject {
    name: String,
    revision: String,
}

pub fn validate(root: &Path, namespace: Option<&str>, profile: &str) -> Result<Secretspec> {
    let text =
        std::fs::read_to_string(root.join("secretspec.toml")).context("read secretspec.toml")?;
    let spec: Secretspec = crate::document::parse(&text, "secretspec.toml")?;
    ensure!(
        spec.project.revision == "1.0",
        "secretspec project revision must be 1.0"
    );
    let namespace =
        namespace.context("governance does not define a secret namespace for this repository")?;
    ensure!(
        spec.project.name == namespace,
        "secretspec project name differs from the governance secret namespace; update [project].name"
    );
    ensure!(
        spec.profiles.contains_key(profile),
        "secretspec does not declare profile {profile}"
    );
    Ok(spec)
}

fn devenv(root: &Path, args: &ContextArgs) -> Command {
    let mut command = Command::new("devenv");
    command.current_dir(root);
    if let Some(profile) = &args.profile {
        command.args(["--secretspec-profile", profile]);
    }
    if let Some(provider) = &args.provider {
        command.args(["--secretspec-provider", provider]);
    }
    if let Some(reason) = &args.reason {
        command.env("SECRETSPEC_REASON", reason);
    }
    command
}

pub fn bao(command: BaoCommand) -> Result<()> {
    match command {
        BaoCommand::Login => process::run(
            Command::new("nix")
                .args([
                    "shell",
                    "nixpkgs#openbao",
                    "--command",
                    "bao",
                    "login",
                    "-method=oidc",
                    "-no-print",
                ])
                .env("BAO_ADDR", BAO_ADDR),
        ),
        BaoCommand::Status => {
            process::check(
                Command::new("bao")
                    .args(["token", "lookup", "-format=json"])
                    .env("BAO_ADDR", BAO_ADDR),
                Duration::from_secs(8),
            )?;
            println!("OpenBao token accepted; project secret access has not been checked.");
            Ok(())
        }
    }
}

pub fn secrets(command: SecretCommand) -> Result<()> {
    let args = match &command {
        SecretCommand::Check { context, .. } | SecretCommand::Set { context, .. } => context,
    };
    if let SecretCommand::Set {
        from_file: None, ..
    } = &command
    {
        ensure!(
            std::io::stdin().is_terminal(),
            "secret updates require a terminal prompt or explicit --from-file PATH (use - for stdin)"
        );
    }
    let root = repo::root()?;
    let operation = match command {
        SecretCommand::Check { .. } => "check",
        SecretCommand::Set { .. } => "set",
    };
    let mut child = Command::new("secretspec");
    child
        .current_dir(&root)
        .arg(operation)
        .arg("--file")
        .arg(root.join("secretspec.toml"))
        .args([
            "--caller",
            "collar",
            "--caller-version",
            env!("CARGO_PKG_VERSION"),
            "--caller-operation",
            operation,
        ]);
    child
        .arg("--caller-resource")
        .arg(root.join("secretspec.toml"));
    if let Some(profile) = &args.profile {
        child.args(["--profile", profile]);
    }
    if let Some(reason) = &args.reason {
        child.arg(format!("--reason={reason}"));
    }
    if let Some(provider) = &args.provider {
        child.args(["--provider", provider]);
    }
    match &command {
        SecretCommand::Check { scope, json, .. } => {
            child.arg("--no-prompt");
            if *json {
                child.arg("--json");
            } else {
                child.arg("--explain");
            }
            if let Some(scope) = scope {
                child.args(["--scope", scope]);
            }
        }
        SecretCommand::Set {
            name, from_file, ..
        } => {
            if let Some(path) = from_file {
                let path = if path == Path::new("-") {
                    path.clone()
                } else {
                    std::fs::canonicalize(path).context("resolve secret input file")?
                };
                child.arg("--from-file").arg(path);
            }
            child.arg("--").arg(name);
        }
    }
    process::run(&mut child)
}

pub fn dev(command: DevCommand) -> Result<()> {
    if let DevCommand::Down { reason } = command {
        let mut child = Command::new("devenv");
        child.current_dir(repo::root()?).args(["processes", "down"]);
        if let Some(reason) = reason {
            child.env("SECRETSPEC_REASON", reason);
        }
        return process::run(&mut child);
    }
    let args = match &command {
        DevCommand::Shell(context)
        | DevCommand::Run { context, .. }
        | DevCommand::Up { context, .. }
        | DevCommand::Task { context, .. } => context,
        DevCommand::Down { .. } => unreachable!(),
    };
    if matches!(command, DevCommand::Shell(_)) {
        ensure!(
            std::io::stdin().is_terminal(),
            "interactive shell requires a terminal; use collar dev run -- COMMAND for scripts"
        );
    }
    let root = repo::root()?;
    let mut child = devenv(&root, args);
    match &command {
        DevCommand::Shell(_) => {
            child.arg("shell");
        }
        DevCommand::Run { command, .. } => {
            child.args(["shell", "--"]).args(command);
        }
        DevCommand::Up { detach, .. } => {
            child.arg("up");
            if *detach {
                child.arg("-d");
            }
        }
        DevCommand::Task { name, .. } => {
            ensure!(!name.is_empty(), "task name must be nonempty");
            child.args(["tasks", "run", "--", name]);
        }
        DevCommand::Down { .. } => unreachable!(),
    }
    process::run(&mut child)
}

pub fn checks(args: CheckArgs) -> Result<()> {
    let root = repo::root()?;
    let mut child = devenv(&root, &args.context);
    child.args(["shell", "--", "prek", "run"]);
    if let Some(hook) = args.hook {
        ensure!(
            !hook.is_empty() && !hook.starts_with('-'),
            "invalid hook identifier"
        );
        child.arg(hook);
    }
    if args.all {
        child.arg("--all-files");
    }
    if !args.files.is_empty() {
        child.arg("--files");
        let root = std::fs::canonicalize(&root)?;
        for path in args.files {
            let path = std::fs::canonicalize(path).context("resolve hook input file")?;
            let relative = path
                .strip_prefix(&root)
                .context("hook input is outside the project checkout")?;
            ensure!(path.is_file(), "hook input must be a file");
            child.arg(Path::new(".").join(relative));
        }
    }
    process::run(&mut child)
}
