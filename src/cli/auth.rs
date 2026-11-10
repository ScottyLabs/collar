use anyhow::Result;
use clap::Subcommand;

use crate::{client::host_url, config::Config, environment};

#[derive(Subcommand)]
pub(super) enum AuthCommand {
    /// Sign in through your browser and save the verified credential.
    #[command(
        after_long_help = "Examples:\n  collar auth login\n  collar auth login --token\n\nBrowser sign-in is the default; uses Keycloak and the Forgejo broker. Git SSH authentication is separate. Headless sessions can use COLLAR_HOST and COLLAR_TOKEN supplied through a secret environment."
    )]
    Login {
        /// Read a personal access token from stdin instead of opening a browser.
        #[arg(long)]
        with_token: bool,
        /// Use a hidden personal-access-token prompt instead of browser SSO.
        #[arg(long, conflicts_with_all = ["with_token", "web"])]
        token: bool,
        /// Start a fresh browser sign-in instead of reusing the saved SSO session.
        #[arg(long, conflicts_with_all = ["with_token", "token"])]
        web: bool,
    },
    /// Revoke this host's SSO session and remove its saved credentials.
    Logout,
    /// Sign in to OpenBao for project secrets; separate from Forgejo login.
    Bao {
        #[command(subcommand)]
        command: environment::BaoCommand,
    },
    /// Verify Forgejo access and Grafana access for a saved SSO session.
    Status,
}

pub(super) fn run(command: AuthCommand, config: &mut Config, host: Option<&str>) -> Result<()> {
    match command {
        AuthCommand::Login {
            with_token,
            token,
            web,
        } => {
            let origin = host_url(host.unwrap_or(config.host()))?;
            crate::auth::login(config, origin, with_token, token, web)
        }
        AuthCommand::Logout => {
            let origin = host_url(host.unwrap_or(config.host()))?;
            crate::auth::logout(config, origin)
        }
        AuthCommand::Bao { command } => environment::bao(command),
        AuthCommand::Status => {
            crate::auth::status(config, host_url(host.unwrap_or(config.host()))?)
        }
    }
}
