use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command as ProcessCommand;

use anyhow::{Context, Result, ensure};
use clap::{Args, Subcommand};
use serde::Serialize;

use crate::output::Output;
use crate::project;
use crate::source::{Kind, Sources};

#[derive(Subcommand)]
pub enum Command {
    /// Run the governance repository's canonical validator in a working checkout.
    Validate(ValidateArgs),
    /// Run a governance CLI command in the governance development shell.
    Run(RunArgs),
    /// Show declared service access and the platform entry points.
    ServiceAccess(ServiceAccessArgs),
}

#[derive(Args)]
pub struct RunArgs {
    /// Governance working checkout.
    #[arg(long, env = "COLLAR_GOVERNANCE", required = true)]
    checkout: PathBuf,
    /// Arguments passed literally to the canonical governance CLI.
    #[arg(required = true, allow_hyphen_values = true, last = true)]
    arguments: Vec<OsString>,
}

#[derive(Args)]
pub struct ValidateArgs {
    /// Governance working checkout; required because source snapshots are read-only.
    #[arg(long, env = "COLLAR_GOVERNANCE", required = true)]
    checkout: PathBuf,
}

#[derive(Args)]
pub struct ServiceAccessArgs {
    /// Read a supplied governance checkout instead of the canonical snapshot.
    #[arg(long, env = "COLLAR_GOVERNANCE")]
    checkout: Option<PathBuf>,
    /// Read the cached canonical governance snapshot without refreshing it.
    #[arg(long)]
    offline: bool,
    #[command(flatten)]
    output: Output,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FeatureAccess {
    name: String,
    declaration: toml::Value,
    platform: &'static str,
    url: Option<&'static str>,
    guidance: &'static str,
}

const FEATURES: &[(&str, &str, Option<&str>, &str)] = &[
    (
        "ai_gateway",
        "LiteLLM",
        None,
        "Use the declared dev, preview, prod, or staging secret profile; governance controls budgets and key limits.",
    ),
    (
        "cdn",
        "Garage CDN",
        None,
        "Use the declared S3 endpoint, bucket, and credentials through secretspec; no URL is implied by this feature.",
    ),
    (
        "docs",
        "Kennel docs deployment",
        Some("https://docs.kennel.scottylabs.org/guides/features.html"),
        "Follow Kennel's feature and deployment guides; the feature alone does not determine a site URL.",
    ),
    (
        "kennel",
        "Kennel deployment",
        Some("https://docs.kennel.scottylabs.org/guides/previews.html"),
        "Run collar kennel status for a pull request; Kennel reports preview URLs in its Forgejo PR comment.",
    ),
    (
        "mobile_app",
        "Google Play and Apple App Store Connect",
        Some("https://play.google.com/console/"),
        "Link Google and, when needed, Apple accounts in Keycloak first. Apple invitations require Share My Email.",
    ),
    (
        "oidc_client",
        "Keycloak",
        Some("https://idp.scottylabs.org/"),
        "Use the declared profile settings through secretspec. Additional OIDC groups are created without managed membership.",
    ),
    (
        "posthog",
        "PostHog",
        Some("https://us.posthog.com/"),
        "Use the prod host and project key through secretspec; governance provisions the project.",
    ),
    (
        "sentry",
        "Sentry",
        Some("https://scottylabs.sentry.io/"),
        "Use the prod DSN through secretspec; governance provisions the project.",
    ),
];

pub fn run(
    command: Command,
    sources: &Sources,
    repository: Option<&str>,
    host: Option<&str>,
) -> Result<()> {
    match command {
        Command::Validate(args) => validate(&args.checkout),
        Command::Run(args) => run_canonical(&args.checkout, &args.arguments),
        Command::ServiceAccess(args) => {
            let mut fields = project::Project::FIELDS.to_vec();
            fields.push("dataSource");
            args.output.validate_fields(&fields)?;
            let repo = sources.config.repo(repository, host)?;
            let source =
                sources.load_at(Kind::Governance, args.checkout, args.offline, &repo.origin)?;
            let project = project::resolve(&source.directory, &repo)?;
            let features = project
                .features
                .iter()
                .map(|(name, declaration)| {
                    let (platform, url, guidance) = FEATURES
                        .iter()
                        .find(|(feature, _, _, _)| *feature == name)
                        .map(|(_, platform, url, guidance)| (*platform, *url, *guidance))
                        .unwrap_or((
                            "Unmapped governance feature",
                            None,
                            "See the governance team declaration and repository documentation for this feature.",
                        ));
                    FeatureAccess {
                        name: name.clone(),
                        declaration: declaration.clone(),
                        platform,
                        url,
                        guidance,
                    }
                })
                .collect::<Vec<_>>();
            let mut value = serde_json::to_value(&project)?;
            value["features"] = serde_json::to_value(&features)?;
            let value = source.attach(&value)?;
            args.output.print(&value, || {
                println!("{}", source.describe());
                project.print();
                for feature in &features {
                    println!(
                        "{}: {}{}\n  {}",
                        feature.name,
                        feature.platform,
                        feature
                            .url
                            .map(|url| format!(" ({url})"))
                            .unwrap_or_default(),
                        feature.guidance
                    );
                }
            })
        }
    }
}

fn validate(checkout: &std::path::Path) -> Result<()> {
    let checkout = governance_checkout(checkout)?;
    let mut command = ProcessCommand::new("cargo");
    command
        .args(["run", "-p", "governance", "--", "validate"])
        .current_dir(&checkout);
    crate::process::run(&mut command).context("run canonical governance validator")
}

fn run_canonical(checkout: &std::path::Path, arguments: &[OsString]) -> Result<()> {
    let checkout = governance_checkout(checkout)?;
    let mut command = ProcessCommand::new("devenv");
    command
        .args(["shell", "--", "cargo", "run", "-p", "governance", "--"])
        .args(arguments)
        .current_dir(checkout);
    crate::process::run(&mut command).context("run canonical governance CLI in devenv shell")
}

fn governance_checkout(checkout: &std::path::Path) -> Result<PathBuf> {
    let checkout = checkout
        .canonicalize()
        .with_context(|| format!("resolve governance checkout {}", checkout.display()))?;
    ensure!(
        checkout.join("Cargo.toml").is_file()
            && checkout.join("crates/governance-cli/Cargo.toml").is_file()
            && checkout.join("data/org.toml").is_file(),
        "{} is not a governance working checkout (expected workspace Cargo.toml, crates/governance-cli, and data/org.toml)",
        checkout.display()
    );
    Ok(checkout)
}
