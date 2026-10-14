mod auth;
mod changelog;
mod checkout;
mod cli;
mod client;
mod config;
mod diff;
mod doctor;
mod document;
mod environment;
mod governance;
mod infra;
mod output;
mod pr;
mod process;
mod project;
mod repo;
mod rfc;
mod source;
mod tui;

use clap::Parser;
use std::process::ExitCode;

fn main() -> ExitCode {
    match cli::run(cli::Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            if error.is::<tui::Cancelled>() {
                ExitCode::from(130)
            } else {
                error
                    .downcast_ref::<process::ToolExit>()
                    .map_or(ExitCode::FAILURE, |exit| ExitCode::from(exit.code()))
            }
        }
    }
}
