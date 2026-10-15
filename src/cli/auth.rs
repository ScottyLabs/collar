use anyhow::Result;
use clap::Subcommand;
use reqwest::Method;

use crate::{auth::api, client::host_url, config::Config, environment, pr};

#[derive(Subcommand)]
pub(super) enum AuthCommand {
    /// Sign in through your browser and save the verified credential.
    #[command(
        after_long_help = "Examples:\n  collar auth login\n  collar auth login --token\n\nBrowser sign-in is the default; choose university SSO in Forgejo. Git SSH authentication is separate. Headless sessions can use COLLAR_HOST and COLLAR_TOKEN supplied through a secret environment."
    )]
    Login {
        /// Read a personal access token from stdin instead of opening a browser.
        #[arg(long)]
        with_token: bool,
        /// Use a hidden personal-access-token prompt instead of browser SSO.
        #[arg(long, conflicts_with_all = ["with_token", "web"])]
        token: bool,
        /// Use browser SSO explicitly (the default login method).
        #[arg(long, conflicts_with_all = ["with_token", "token"])]
        web: bool,
    },
    /// Remove this host's saved credential without revoking it on the server.
    Logout,
    /// Sign in to OpenBao for project secrets; separate from Forgejo login.
    Bao {
        #[command(subcommand)]
        command: environment::BaoCommand,
    },
    /// Verify your API credential against the selected forge.
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
            let api = api(host_url(host.unwrap_or(config.host()))?, config)?;
            api.require_token()?;
            let user: pr::User = api.json(Method::GET, &["user"], &[], None)?;
            println!("Logged in to {} as {}", api.origin(), user.login);
            Ok(())
        }
    }
}
