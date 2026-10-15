use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Subcommand};
use serde::Serialize;

use crate::output::Output;
use crate::project;
use crate::source::{Kind, Sources};

#[derive(Subcommand)]
pub enum Command {
    /// Show declared service access and the platform entry points.
    ServiceAccess(ServiceAccessArgs),
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
