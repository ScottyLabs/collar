mod cli;
mod client;
mod config;
mod diff;
mod governance;
mod infra;
mod output;
mod pr;
mod project;
mod repo;

use clap::Parser;
use std::process::ExitCode;

fn main() -> ExitCode {
    match cli::run(cli::Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}
