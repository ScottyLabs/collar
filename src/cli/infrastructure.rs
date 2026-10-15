use std::path::PathBuf;

use anyhow::Result;
use clap::Subcommand;

use crate::{
    infra, infra_ops,
    output::Output,
    source::{Kind, Sources},
    ui,
};

#[derive(Subcommand)]
pub(super) enum InfraCommand {
    #[command(flatten)]
    Workflows(infra_ops::CommandKind),
    /// List declared infrastructure host configurations.
    Hosts {
        /// Inspect local infrastructure changes instead of the latest canonical revision.
        #[arg(long, env = "COLLAR_INFRASTRUCTURE")]
        checkout: Option<PathBuf>,
        /// Read an existing snapshot without refreshing it.
        #[arg(long)]
        offline: bool,
        #[command(flatten)]
        output: Output,
    },
    /// Find the source configuration for a service across hosts.
    Service {
        name: String,
        /// Inspect local infrastructure changes instead of the latest canonical revision.
        #[arg(long, env = "COLLAR_INFRASTRUCTURE")]
        checkout: Option<PathBuf>,
        /// Read an existing snapshot without refreshing it.
        #[arg(long)]
        offline: bool,
        #[command(flatten)]
        output: Output,
    },
}

pub(super) fn run(command: InfraCommand, sources: &Sources<'_>) -> Result<()> {
    match command {
        InfraCommand::Workflows(command) => infra_ops::run(command, sources),
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
    }
}
