mod auth;
mod cli;
mod client;
mod config;
mod diff;
mod doctor;
mod document;
mod governance;
mod infra;
mod output;
mod pr;
mod process;
mod project;
mod repo;
mod rfc;

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
