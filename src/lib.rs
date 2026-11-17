#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable
    )
)]
//! Authenticated Forgejo access shared with the Collar CLI.
//!
//! Reuse one client for a sequence of requests. Constructing a client does not parse CLI arguments, open a browser, or print output. Credentials are resolved by [`authenticated_api`] using the same host-bound settings as the CLI.
//!
//! ```no_run
//! # fn example() -> anyhow::Result<()> {
//! use collar::{Config, Method, authenticated_api, host_url};
//!
//! let config = Config::load(None, false, false)?;
//! let api = authenticated_api(host_url("git.cmu.dev")?, &config)?;
//! let issues: Vec<serde_json::Value> = api.list(
//!     &["repos", "ScottyLabs", "collar", "issues"],
//!     &[("state".into(), "open".into())],
//!     50,
//! )?;
//! let _user: serde_json::Value = api.json(Method::GET, &["user"], &[], None)?;
//! # Ok(())
//! # }
//! ```

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
mod issues;
mod kennel;
mod kennel_api;
mod mcp;
mod nixbot;
mod output;
pub mod pr;
mod process;
mod project;
mod releases;
mod repo;
mod rfc;
mod skill;
mod source;
mod status;
mod tui;
mod ui;

pub mod sso;

pub use auth::api as authenticated_api;
pub use client::{Api, DEFAULT_HOST, HttpStatus, ReadCache, host_url};
pub use config::Config;
pub use repo::Repo;
pub use reqwest::Method;

use clap::Parser;
use std::process::ExitCode;

/// Run the command-line interface using process arguments and terminal output.
///
/// Library consumers should use the client APIs; this entry point also installs the CLI's SIGPIPE behavior and maps errors to process exit codes.
pub fn run_cli() -> ExitCode {
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
