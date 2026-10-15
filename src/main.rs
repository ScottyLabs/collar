#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable
    )
)]

mod actions;
mod auth;
mod changelog;
mod checkout;
mod cli;
mod client;
mod config;
mod diff;
mod docs;
mod doctor;
mod document;
mod environment;
mod governance;
mod governance_ops;
mod infra;
mod infra_ops;
mod issues;
mod kennel;
mod oauth;
mod output;
mod pr;
mod process;
mod project;
mod releases;
mod repo;
mod rfc;
mod source;
mod status;
mod tui;
mod ui;

use clap::Parser;
use std::process::ExitCode;

fn main() -> ExitCode {
    sigpipe::reset();
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
