use std::io::Write;
use std::process::{Command, Output};

use serde_json::{Value, json};
use tempfile::TempDir;

#[path = "cases/inbox.rs"]
mod inbox;

#[path = "cases/reviewers.rs"]
mod reviewers;

#[path = "cases/skill.rs"]
mod skill;

mod support;
use support::*;

#[cfg(unix)]
#[path = "cases/environment.rs"]
mod environment;

#[path = "cases/authoring.rs"]
mod authoring;

#[path = "cases/merging.rs"]
mod merging;

#[path = "cases/checkout.rs"]
mod checkout;

#[path = "cases/auth.rs"]
mod auth;

#[cfg(unix)]
#[cfg(unix)]
#[path = "cases/governance_ops.rs"]
mod governance_ops;

#[path = "cases/kennel.rs"]
mod kennel;

#[path = "cases/kennel_retry.rs"]
mod kennel_retry;

#[path = "cases/issues.rs"]
mod issues;

#[path = "cases/releases.rs"]
mod releases;

#[path = "cases/actions.rs"]
mod actions;

#[path = "cases/clone.rs"]
mod clone;

#[cfg(unix)]
#[path = "cases/oauth.rs"]
mod oauth;

#[path = "cases/docs.rs"]
mod docs;

#[path = "cases/repository.rs"]
mod repository;

#[path = "cases/interface.rs"]
mod interface;

#[path = "cases/reviews.rs"]
mod reviews;

#[path = "cases/api.rs"]
mod api;

#[path = "cases/configuration.rs"]
mod configuration;

#[path = "cases/discovery.rs"]
mod discovery;

#[path = "cases/checks.rs"]
mod checks;

#[cfg(unix)]
#[path = "cases/doctor.rs"]
mod doctor;

#[path = "cases/changelog.rs"]
mod changelog;

#[path = "cases/sources.rs"]
mod sources;
