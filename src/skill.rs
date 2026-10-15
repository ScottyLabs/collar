//! Distribute the same agent skill with every installed Collar binary.

use std::{fs, io::Write, path::PathBuf};

use anyhow::{Context, Result, ensure};
use clap::{Subcommand, ValueEnum};
use directories::BaseDirs;

const CONTENT: &str = include_str!("../docs/skills/collar/SKILL.md");

#[derive(Subcommand)]
pub enum Command {
    /// Install the bundled skill, or replace it explicitly after updating Collar.
    Install {
        /// Skill root directory; Collar creates a collar/SKILL.md beneath it.
        #[arg(conflicts_with = "agent")]
        directory: Option<PathBuf>,
        /// Client whose user skill directory to use; defaults to codex.
        #[arg(long, value_enum)]
        agent: Option<Agent>,
        /// Replace a different installed skill, including local edits.
        #[arg(long)]
        force: bool,
    },
}

#[derive(Clone, Copy, ValueEnum)]
pub enum Agent {
    Codex,
    Claude,
}

pub fn run(command: Option<Command>) -> Result<()> {
    let Some(Command::Install {
        directory,
        agent,
        force,
    }) = command
    else {
        print!("{CONTENT}");
        return Ok(());
    };
    let root = match directory {
        Some(directory) => directory,
        None => {
            let dirs = BaseDirs::new()
                .context("cannot find home directory; specify a skill root directory")?;
            dirs.home_dir().join(match agent.unwrap_or(Agent::Codex) {
                Agent::Codex => ".agents/skills",
                Agent::Claude => ".claude/skills",
            })
        }
    };
    let directory = root.join("collar");
    fs::create_dir_all(&directory).with_context(|| format!("create {}", directory.display()))?;
    let path = directory.join("SKILL.md");
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(mut file) => file
            .write_all(CONTENT.as_bytes())
            .context("write Collar skill")?,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            ensure!(
                fs::symlink_metadata(&path)?.file_type().is_file(),
                "skill destination must be a regular file: {}",
                path.display()
            );
            if fs::read(&path)? == CONTENT.as_bytes() {
                println!("Collar skill is current at {}", path.display());
                return Ok(());
            }
            ensure!(
                force,
                "a different skill exists at {}; use --force to replace it",
                path.display()
            );
            fs::write(&path, CONTENT).context("replace Collar skill")?;
        }
        Err(error) => return Err(error).context("create Collar skill"),
    }
    println!("Installed Collar skill at {}", path.display());
    Ok(())
}
