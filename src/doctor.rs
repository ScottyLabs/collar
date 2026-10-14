use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use anyhow::{Context, Result, ensure};
use clap::Args as ClapArgs;
use reqwest::Method;
use serde::{Deserialize, Serialize};

use crate::config::{Config, Key};
use crate::output::Output;
use crate::{auth, infra, pr, process, project, repo};

const LOGIN: &str = "nix run git+https://git.cmu.dev/ScottyLabs/kennel#login";
const ACCOUNTS: &str =
    "https://idp.scottylabs.org/realms/scottylabs/account/account-security/linked-accounts";

#[derive(ClapArgs)]
pub struct Args {
    /// Skip forge and Bao requests; never resolve secrets.
    #[arg(long)]
    offline: bool,
    #[arg(long, env = "COLLAR_GOVERNANCE")]
    governance: Option<PathBuf>,
    #[arg(long, env = "COLLAR_INFRASTRUCTURE")]
    infrastructure: Option<PathBuf>,
    /// Secretspec profile to inspect without reading its secret values.
    #[arg(long, default_value = "dev")]
    profile: String,
    #[command(flatten)]
    output: Output,
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
enum Status {
    Pass,
    Fail,
    Manual,
    Skipped,
}

#[derive(Serialize)]
struct Check {
    name: String,
    status: Status,
    detail: String,
    action: Option<String>,
}

fn record(checks: &mut Vec<Check>, name: &str, result: Result<String>, action: &str) {
    let (status, detail, action) = match result {
        Ok(detail) => (Status::Pass, detail, None),
        Err(error) => (Status::Fail, format!("{error:#}"), Some(action.into())),
    };
    checks.push(Check {
        name: name.into(),
        status,
        detail,
        action,
    });
}

fn skipped(checks: &mut Vec<Check>, name: &str, detail: &str) {
    checks.push(Check {
        name: name.into(),
        status: Status::Skipped,
        detail: detail.into(),
        action: None,
    });
}

fn git(checkout: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(checkout)
        .args(args)
        .output()
        .context("run Git diagnostic")?;
    ensure!(
        output.status.success(),
        "git {} failed ({})",
        args.join(" "),
        output.status
    );
    Ok(String::from_utf8(output.stdout)
        .context("Git diagnostic is not UTF-8")?
        .trim()
        .to_owned())
}

fn checkout_revision(checkout: &Path) -> Result<String> {
    let root = git(checkout, &["rev-parse", "--show-toplevel"])?;
    ensure!(
        std::fs::canonicalize(root)? == std::fs::canonicalize(checkout)?,
        "configured checkout is not a Git repository root"
    );
    let head = git(checkout, &["rev-parse", "HEAD"])?;
    let changes = git(checkout, &["status", "--porcelain"])?;
    ensure!(
        changes.is_empty(),
        "checkout has local changes at {head}; declarations include those changes"
    );
    let divergence = git(
        checkout,
        &["rev-list", "--left-right", "--count", "HEAD...@{upstream}"],
    )?;
    let counts: Vec<_> = divergence.split_whitespace().collect();
    ensure!(
        counts == ["0", "0"],
        "checkout differs from its last-fetched upstream (ahead/behind: {divergence})"
    );
    Ok(format!(
        "clean at {head}; matches last-fetched upstream, remote freshness is not verified"
    ))
}

#[derive(Deserialize)]
struct Secretspec {
    project: SecretProject,
    profiles: BTreeMap<String, toml::Value>,
}

#[derive(Deserialize)]
struct SecretProject {
    name: String,
    revision: String,
}

fn secretspec(root: &Path, namespace: Option<&str>, profile: &str) -> Result<String> {
    let text =
        std::fs::read_to_string(root.join("secretspec.toml")).context("read secretspec.toml")?;
    let spec: Secretspec = crate::document::parse(&text, "secretspec.toml")?;
    ensure!(
        spec.project.revision == "1.0",
        "secretspec project revision must be 1.0"
    );
    let namespace =
        namespace.context("governance does not define a secret namespace for this repository")?;
    ensure!(
        spec.project.name == namespace,
        "secretspec project name differs from the governance secret namespace; update [project].name"
    );
    ensure!(
        spec.profiles.contains_key(profile),
        "secretspec does not declare profile {profile}"
    );
    Ok(format!(
        "namespace {namespace}, profile {profile}; secret values and provider access were not checked"
    ))
}

pub fn run(args: Args, config: &Config, selection: Option<&str>, host: Option<&str>) -> Result<()> {
    args.output
        .validate_fields(&["name", "status", "detail", "action"])?;
    let mut checks = Vec::new();
    for (tool, version) in [
        ("git", "--version"),
        ("nix", "--version"),
        ("devenv", "--version"),
        ("bao", "version"),
        ("secretspec", "--version"),
    ] {
        record(
            &mut checks,
            &format!("tool:{tool}"),
            process::check(Command::new(tool).arg(version), Duration::from_secs(5))
                .map(|()| "available".into()),
            "Install Nix/devenv and enter the project's devenv shell; ScottyLabs shared tooling supplies Bao and secretspec.",
        );
    }
    let root = repo::root();
    record(
        &mut checks,
        "checkout",
        root.as_ref()
            .map(|path| path.display().to_string())
            .map_err(|error| anyhow::anyhow!("{error:#}")),
        "Run doctor inside a project Git checkout.",
    );
    if let Ok(root) = &root {
        for key in ["user.name", "user.email"] {
            record(
                &mut checks,
                &format!("git:{key}"),
                git(root, &["config", "--get", key]).and_then(|value| {
                    ensure!(!value.is_empty(), "Git identity is empty");
                    Ok("configured".into())
                }),
                "Configure your Git author name and email with git config.",
            );
        }
        record(
            &mut checks,
            "devenv",
            (|| {
                for file in ["devenv.nix", "devenv.yaml", "flake.nix"] {
                    ensure!(root.join(file).is_file(), "project is missing {file}");
                }
                Ok("project files present; evaluation and shell readiness are not verified".into())
            })(),
            "Follow the Kennel devenv setup guide: https://docs.kennel.scottylabs.org/guides/deploying.html",
        );
    }
    record(
        &mut checks,
        "ssh-agent",
        process::check(Command::new("ssh-add").arg("-l"), Duration::from_secs(5))
            .map(|()| "agent has keys; forge authentication is not verified".into()),
        "Load your SSH key with ssh-add; register its public key in your forge's SSH settings.",
    );
    let selected = config.repo(selection, host);
    record(
        &mut checks,
        "repository",
        selected
            .as_ref()
            .map(|repo| format!("{}{}/{}", repo.origin, repo.owner, repo.name))
            .map_err(|error| anyhow::anyhow!("{error:#}")),
        "Configure upstream/origin or select the canonical repository with -R.",
    );
    let governance = config.checkout(args.governance, Key::Governance);
    let context = (|| {
        project::resolve(
            governance
                .as_ref()
                .map_err(|error| anyhow::anyhow!("{error:#}"))?,
            selected
                .as_ref()
                .map_err(|error| anyhow::anyhow!("{error:#}"))?,
        )
    })();
    record(
        &mut checks,
        "governance",
        context
            .as_ref()
            .map(|project| format!("project {}, team {}", project.slug, project.team))
            .map_err(|error| anyhow::anyhow!("{error:#}")),
        "Configure a current canonical governance checkout with collar config set governance PATH.",
    );
    if let Ok(checkout) = governance {
        record(
            &mut checks,
            "governance-revision",
            checkout_revision(&checkout),
            "Inspect local changes and fetch/rebase the governance checkout; doctor never updates it.",
        );
    }
    if let (Ok(root), Ok(project)) = (&root, &context) {
        record(
            &mut checks,
            "secretspec",
            secretspec(root, project.secret_namespace.as_deref(), &args.profile),
            "Align secretspec.toml with governance, declare the selected profile, then run secretspec check explicitly.",
        );
    } else {
        skipped(
            &mut checks,
            "secretspec",
            "requires a local project checkout and resolved governance namespace",
        );
    }
    let infrastructure = config.checkout(args.infrastructure, Key::Infrastructure);
    record(
        &mut checks,
        "infrastructure",
        infrastructure
            .as_ref()
            .map_err(|error| anyhow::anyhow!("{error:#}"))
            .and_then(|path| infra::hosts(path))
            .map(|hosts| {
                format!(
                    "{} host source configurations; evaluation and health are not verified",
                    hosts.len()
                )
            }),
        "Configure infrastructure with collar config set infrastructure PATH.",
    );
    if let Ok(checkout) = infrastructure {
        record(
            &mut checks,
            "infrastructure-revision",
            checkout_revision(&checkout),
            "Inspect local changes and fetch/rebase the infrastructure checkout; doctor never updates it.",
        );
    }
    checks.push(Check {
        name: "linked-accounts".into(),
        status: Status::Manual,
        detail: "Keycloak account linking and applied group membership require verification".into(),
        action: Some(
            context
                .as_ref()
                .map(|project| project.linked_accounts_url.to_string())
                .unwrap_or_else(|_| ACCOUNTS.into()),
        ),
    });
    if args.offline {
        for name in ["forge", "forge-auth", "bao-auth"] {
            skipped(
                &mut checks,
                name,
                "offline mode; remote access was not checked",
            );
        }
    } else {
        match selected.and_then(|repo| auth::api(repo.origin, config)) {
            Ok(api) => {
                #[derive(Deserialize)]
                struct Version {
                    version: String,
                }
                record(
                    &mut checks,
                    "forge",
                    api.json::<Version>(Method::GET, &["version"], &[], None)
                        .map(|version| format!("Forgejo {} reachable", version.version)),
                    "Check the selected forge host and network connection.",
                );
                record(
                    &mut checks,
                    "forge-auth",
                    api.require_token()
                        .and_then(|()| api.json::<pr::User>(Method::GET, &["user"], &[], None))
                        .map(|user| format!("authenticated as {}", user.login)),
                    "Set a host-bound COLLAR_TOKEN and run collar auth status.",
                );
            }
            Err(error) => record(
                &mut checks,
                "forge",
                Err(error),
                "Fix repository selection and host-bound authentication configuration.",
            ),
        }
        record(
            &mut checks,
            "bao-auth",
            process::check(
                Command::new("bao")
                    .args(["token", "lookup", "-format=json"])
                    .env("BAO_ADDR", "https://secrets.scottylabs.org"),
                Duration::from_secs(8),
            )
            .map(|()| {
                "token accepted by ScottyLabs OpenBao; project secret access is not verified".into()
            }),
            LOGIN,
        );
    }
    let failed = checks
        .iter()
        .filter(|check| matches!(check.status, Status::Fail))
        .count();
    args.output.print(&checks, || {
        for check in &checks {
            let status = match check.status {
                Status::Pass => "PASS",
                Status::Fail => "FAIL",
                Status::Manual => "MANUAL",
                Status::Skipped => "SKIPPED",
            };
            println!("{status}\t{}\t{}", check.name, check.detail);
            if let Some(action) = &check.action {
                println!("  {action}");
            }
        }
    })?;
    ensure!(
        failed == 0,
        "{failed} diagnostics failed; complete manual checks before relying on access"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_namespace_profile_and_revision_without_echoing_values() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("secretspec.toml");
        std::fs::write(
            &path,
            "[project]\nname = 'project-slug'\nrevision = '1.0'\n[profiles.dev]\n",
        )
        .unwrap();
        assert!(secretspec(temp.path(), Some("project-slug"), "dev").is_ok());
        assert!(
            secretspec(temp.path(), Some("repo-name"), "dev")
                .unwrap_err()
                .to_string()
                .contains("differs")
        );
        assert!(
            secretspec(temp.path(), Some("project-slug"), "prod")
                .unwrap_err()
                .to_string()
                .contains("profile prod")
        );
        assert!(secretspec(temp.path(), None, "dev").is_err());
        std::fs::write(
            &path,
            "[project]\nname = ['never-echo-this-secret']\nrevision = '1.0'\n[profiles.dev]\n",
        )
        .unwrap();
        let error = secretspec(temp.path(), Some("project-slug"), "dev").unwrap_err();
        assert!(!format!("{error:#}").contains("never-echo-this-secret"));
    }
}
