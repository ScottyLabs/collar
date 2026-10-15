use std::path::PathBuf;

use anyhow::{Result, ensure};
use clap::Subcommand;

use crate::{
    client::host_url,
    config::Config,
    governance, governance_ops,
    output::Output,
    project,
    source::{Kind, Sources},
    tui, ui,
};

#[derive(Subcommand)]
pub(super) enum ContextCommand {
    /// Pick a project repository interactively; emits its context.
    Select {
        /// Inspect local governance changes instead of the latest canonical revision.
        #[arg(long, env = "COLLAR_GOVERNANCE")]
        checkout: Option<PathBuf>,
        #[arg(long)]
        offline: bool,
        #[command(flatten)]
        output: Output,
    },
    /// Show declared ownership, features, and the secret namespace.
    View {
        /// Repository as [HOST/][OWNER/]REPO; defaults to the current repository.
        repository: Option<String>,
        /// Inspect local governance changes instead of the latest canonical revision.
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
pub(super) enum GovernanceCommand {
    #[command(flatten)]
    Workflows(governance_ops::Command),
    /// List governed projects or inspect one by name.
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
}

#[derive(Subcommand)]
pub(super) enum ProjectCommand {
    /// List repositories with their owning projects and teams.
    List {
        /// Inspect local governance changes instead of the latest canonical revision.
        #[arg(long, env = "COLLAR_GOVERNANCE")]
        checkout: Option<PathBuf>,
        /// Read an existing snapshot without refreshing it.
        #[arg(long)]
        offline: bool,
        #[command(flatten)]
        output: Output,
    },
    /// Inspect a project selected by slug or repository name.
    View {
        /// Project slug or repository name.
        name: String,
        /// Inspect local governance changes instead of the latest canonical revision.
        #[arg(long, env = "COLLAR_GOVERNANCE")]
        checkout: Option<PathBuf>,
        /// Read an existing snapshot without refreshing it.
        #[arg(long)]
        offline: bool,
        #[command(flatten)]
        output: Output,
    },
}

pub(super) fn run_context(
    command: ContextCommand,
    sources: &Sources<'_>,
    config: &Config,
    selection: Option<&str>,
    host: Option<&str>,
) -> Result<()> {
    match command {
        ContextCommand::Select {
            checkout,
            offline,
            output,
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
            if let Some(host) = host {
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
        ContextCommand::View {
            repository,
            checkout,
            offline,
            output,
        } => {
            let mut fields = project::Project::FIELDS.to_vec();
            fields.push("dataSource");
            output.validate_fields(&fields)?;
            let repo = config.repo(repository.as_deref().or(selection), host)?;
            let source = sources.load_at(Kind::Governance, checkout, offline, &repo.origin)?;
            let project = project::resolve(&source.directory, &repo)?;
            output.print(&source.attach(&project)?, || {
                project.print();
                ui::detail("Source", &source.describe());
            })
        }
    }
}

pub(super) fn run_governance(
    command: GovernanceCommand,
    sources: &Sources<'_>,
    selection: Option<&str>,
    host: Option<&str>,
) -> Result<()> {
    match command {
        GovernanceCommand::Workflows(command) => {
            governance_ops::run(command, sources, selection, host)
        }
        GovernanceCommand::Project { command } => match command {
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
    }
}
