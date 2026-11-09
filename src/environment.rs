use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io::{self, IsTerminal};
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
    /// Secret profile; the shared project shell defaults to dev.
    #[arg(long, env = "SECRETSPEC_PROFILE")]
    profile: Option<String>,
    /// Audit reason forwarded unchanged to secretspec
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
    /// Start local processes and wait for readiness.
    #[command(
        long_about = "Start local processes through Pitchfork, waiting for readiness and dependencies. Omit names to start every project process. Shell secret options do not override profiles selected by process scripts."
    )]
    Up(ProcessArgs),
    /// Stop local processes in reverse dependency order.
    Down {
        /// Project process names; omit to select every local process.
        #[arg(value_parser = process_name)]
        names: Vec<String>,
    },
    /// Restart local processes through Pitchfork.
    Restart(ProcessArgs),
    /// List this project's processes or inspect one process.
    Status {
        #[arg(value_parser = process_name)]
        name: Option<String>,
        /// Emit Pitchfork's native JSON.
        #[arg(long)]
        json: bool,
    },
    /// Read a local process's logs.
    Logs {
        #[arg(value_parser = process_name)]
        name: String,
        /// Maximum log lines to read before following, if requested.
        #[arg(long, default_value_t = 50, value_parser = clap::value_parser!(u64).range(1..=10000))]
        tail: u64,
        /// Follow new output until interrupted.
        #[arg(long, conflicts_with = "json")]
        follow: bool,
        /// Emit Pitchfork's native JSON.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Args)]
pub struct ProcessArgs {
    /// Project process names; omit to select every local process.
    #[arg(value_parser = process_name)]
    names: Vec<String>,
    #[command(flatten)]
    context: ContextArgs,
}

fn process_name(value: &str) -> std::result::Result<String, String> {
    if !value.is_empty()
        && !value.starts_with('-')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte))
    {
        Ok(value.into())
    } else {
        Err("use an unqualified project process name containing letters, digits, _, ., or -".into())
    }
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
    let text = fs::read_to_string(root.join("secretspec.toml")).context("read secretspec.toml")?;
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

fn develop(root: &Path, args: &ContextArgs) -> Command {
    let mut command = Command::new("nix");
    command.current_dir(root).arg("develop");
    for (name, value) in [
        ("SECRETSPEC_PROFILE", &args.profile),
        ("SECRETSPEC_PROVIDER", &args.provider),
        ("SECRETSPEC_REASON", &args.reason),
    ] {
        if let Some(value) = value {
            command.env(name, value);
        }
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
            io::stdin().is_terminal(),
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
                    fs::canonicalize(path).context("resolve secret input file")?
                };
                child.arg("--from-file").arg(path);
            }
            child.arg("--").arg(name);
        }
    }
    process::run(&mut child)
}

pub fn dev(command: DevCommand) -> Result<()> {
    match command {
        DevCommand::Shell(context) => dev_shell(&context),
        DevCommand::Run { context, command } => dev_run(&context, command),
        DevCommand::Up(args) => {
            let mut command = vec!["pitchfork".into()];
            command.extend(pitchfork("start", args.names));
            dev_run(&args.context, command)
        }
        DevCommand::Down { names } => local_process(pitchfork("stop", names)),
        DevCommand::Restart(args) => {
            let mut command = vec!["pitchfork".into()];
            command.extend(pitchfork("restart", args.names));
            dev_run(&args.context, command)
        }
        DevCommand::Status { name, json } => {
            let mut args = match name {
                Some(name) => vec!["status".into(), name.into()],
                None => vec!["list".into(), "--project".into()],
            };
            if json {
                args.push("--json".into());
            }
            local_process(args)
        }
        DevCommand::Logs {
            name,
            tail,
            follow,
            json,
        } => {
            let mut args = vec![
                "logs".into(),
                name.into(),
                "-n".into(),
                tail.to_string().into(),
                "--no-pager".into(),
            ];
            if follow {
                args.push("--follow".into());
            }
            if json {
                args.push("--json".into());
            }
            local_process(args)
        }
    }
}

fn dev_shell(context: &ContextArgs) -> Result<()> {
    ensure!(
        io::stdin().is_terminal(),
        "interactive shell requires a terminal; use collar dev run -- COMMAND for scripts"
    );
    process::run(&mut develop(&repo::root()?, context))
}

fn dev_run(context: &ContextArgs, command: Vec<OsString>) -> Result<()> {
    process::run(
        develop(&repo::root()?, context)
            .arg("--command")
            .args(command),
    )
}

fn pitchfork(action: &str, names: Vec<String>) -> Vec<OsString> {
    let mut args = vec![action.into()];
    if names.is_empty() {
        args.push("--local".into());
    } else {
        args.extend(names.into_iter().map(OsString::from));
    }
    args
}

fn local_process(args: Vec<OsString>) -> Result<()> {
    process::run(
        Command::new("pitchfork")
            .current_dir(repo::root()?)
            .args(args),
    )
}

pub fn checks(args: CheckArgs) -> Result<()> {
    let root = repo::root()?;
    let mut child = develop(&root, &args.context);
    child.args(["--command", "prek", "run"]);
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
        let root = fs::canonicalize(&root)?;
        for path in args.files {
            let path = fs::canonicalize(path).context("resolve hook input file")?;
            let relative = path
                .strip_prefix(&root)
                .context("hook input is outside the project checkout")?;
            ensure!(path.is_file(), "hook input must be a file");
            child.arg(Path::new(".").join(relative));
        }
    }
    process::run(&mut child)
}
