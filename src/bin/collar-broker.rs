use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;

#[derive(Parser)]
#[command(about = "Relay verified Collar sessions to Keycloak's cmu-dev broker")]
struct Args {
    #[arg(long, default_value = "127.0.0.1:8094")]
    listen: SocketAddr,
    #[arg(long)]
    client_secret_file: PathBuf,
}

fn run() -> Result<()> {
    let args = Args::parse();
    let secret =
        std::fs::read_to_string(args.client_secret_file).context("read broker client secret")?;
    collar::sso::relay::run(
        args.listen,
        collar::sso::Endpoints::from_env()?,
        secret.trim_end_matches(['\r', '\n']).to_owned(),
    )
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(_) => {
            eprintln!("collar-broker: startup or worker failure");
            std::process::ExitCode::FAILURE
        }
    }
}
