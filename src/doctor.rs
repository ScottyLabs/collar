//! Aggregate local tooling, declared project context, and remote access diagnostics.

use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use anyhow::{Context, Result, ensure};
use clap::Args as ClapArgs;
use reqwest::Method;
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::output::Output;
use crate::repo::Repo;
use crate::source::{Kind, Source, Sources};
use crate::{auth, pr, process, project, repo};

const LOGIN: &str = "nix run git+https://git.cmu.dev/ScottyLabs/kennel#login";
const ACCOUNTS: &str =
    "https://idp.scottylabs.org/realms/scottylabs/account/account-security/linked-accounts";
const CACHE: &str = "https://scottylabs.cachix.org";
const CACHE_KEY: &str = "scottylabs.cachix.org-1:hajjEX5SLi/Y7yYloiXTt2IOr3towcTGRhMh1vu6Tjg=";
const NIX_COMMAND: &str = "extra-experimental-features = nix-command";

#[derive(ClapArgs)]
pub struct Args {
    /// Skip forge and Bao requests; never resolve secrets.
    #[arg(long)]
    offline: bool,
    #[arg(long, env = "COLLAR_GOVERNANCE")]
    governance: Option<PathBuf>,
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

fn record(
    checks: &mut Vec<Check>,
    name: &str,
    result: std::result::Result<String, impl Display>,
    action: &str,
) {
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

fn secretspec(root: &Path, namespace: Option<&str>, profile: &str) -> Result<String> {
    crate::environment::validate(root, namespace, profile)?;
    Ok(format!(
        "namespace {}, profile {profile}; secret values and provider access were not checked",
        namespace.context("project has no namespace")?
    ))
}

fn flake(root: &Path) -> Result<String> {
    ensure!(
        root.join("flake.nix").is_file(),
        "project is missing flake.nix"
    );
    Ok("flake.nix present; evaluation and shell readiness are not verified".into())
}

fn nix_json(args: &[&str], system_only: bool) -> Result<serde_json::Value> {
    let mut command = Command::new("nix");
    command.args(args).stdin(std::process::Stdio::null());
    if system_only {
        command
            .env("NIX_USER_CONF_FILES", "/dev/null")
            .env("NIX_CONFIG", NIX_COMMAND);
    } else {
        let config = std::env::var("NIX_CONFIG").unwrap_or_default();
        command.env("NIX_CONFIG", format!("{config}\n{NIX_COMMAND}"));
    }
    let output = command.output().context("run Nix diagnostic")?;
    ensure!(
        output.status.success(),
        "nix {} failed ({})",
        args.join(" "),
        output.status
    );
    serde_json::from_slice(&output.stdout).context("Nix diagnostic is not JSON")
}

fn strings(config: &serde_json::Value, setting: &str) -> Vec<String> {
    config[setting]["value"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|value| value.as_str().map(str::to_owned))
        .collect()
}

fn trusted_user() -> Result<bool> {
    let store = nix_json(&["store", "info", "--json"], false)
        .or_else(|_| nix_json(&["store", "ping", "--json"], false))?;
    Ok(store["trusted"].as_bool() == Some(true) || store["trusted"].as_u64() == Some(1))
}

fn accepted_flake_settings(config: &serde_json::Value) -> (Vec<String>, Vec<String>) {
    if config["accept-flake-config"]["value"].as_bool() == Some(true) {
        return (vec![CACHE.into()], vec![CACHE_KEY.into()]);
    }
    let saved = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .and_then(|data| std::fs::read(data.join("nix/trusted-settings.json")).ok())
        .and_then(|saved| serde_json::from_slice::<serde_json::Value>(&saved).ok())
        .unwrap_or_default();
    let approved = |settings: [&str; 2]| -> Vec<String> {
        settings
            .iter()
            .filter_map(|setting| saved[*setting].as_object())
            .flatten()
            .filter(|(_, approved)| approved.as_bool() == Some(true))
            .flat_map(|(values, _)| values.split_whitespace().map(str::to_owned))
            .collect()
    };
    (
        approved(["extra-substituters", "substituters"]),
        approved(["extra-trusted-public-keys", "trusted-public-keys"]),
    )
}

fn daemon_caches() -> Result<(Vec<String>, Vec<String>)> {
    let system = nix_json(&["config", "show", "--json"], true)?;
    let config = nix_json(&["config", "show", "--json"], false)?;
    let (flake_substituters, flake_keys) = accepted_flake_settings(&config);
    let mut substituters = strings(&system, "substituters");
    let mut keys = strings(&system, "trusted-public-keys");
    let requested = strings(&config, "substituters")
        .into_iter()
        .chain(flake_substituters);
    if trusted_user()? {
        substituters.extend(requested);
        keys.extend(
            strings(&config, "trusted-public-keys")
                .into_iter()
                .chain(flake_keys),
        );
    } else {
        let allowed = strings(&system, "trusted-substituters");
        substituters.extend(requested.filter(|substituter| allowed.contains(substituter)));
    }
    let mut seen = std::collections::HashSet::new();
    substituters.retain(|substituter| seen.insert(substituter.clone()));
    Ok((substituters, keys))
}

fn store_hash() -> Option<String> {
    let in_store = |path: PathBuf| {
        std::fs::canonicalize(path)
            .ok()
            .filter(|path| path.starts_with("/nix/store"))
    };
    let path = std::env::current_exe()
        .ok()
        .and_then(in_store)
        .or_else(|| {
            std::env::split_paths(&std::env::var_os("PATH")?)
                .find_map(|directory| in_store(directory.join("collar")))
        })?;
    let entry = path
        .strip_prefix("/nix/store")
        .ok()?
        .iter()
        .next()?
        .to_str()?;
    Some(entry.split_once('-')?.0.to_owned())
}

fn binary_cache(hash: &str) -> Result<String> {
    let (substituters, keys) = daemon_caches()?;
    let names: Vec<&str> = keys
        .iter()
        .filter_map(|key| key.split_once(':').map(|(name, _)| name))
        .collect();
    let client = reqwest::blocking::Client::builder()
        .user_agent(concat!("collar/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(5))
        .build()
        .context("create HTTP client")?;
    for substituter in &substituters {
        let base = substituter
            .split('?')
            .next()
            .unwrap_or_default()
            .trim_end_matches('/');
        if !(base.starts_with("https://") || base.starts_with("http://")) {
            continue;
        }
        let Ok(response) = client.get(format!("{base}/{hash}.narinfo")).send() else {
            continue;
        };
        if !response.status().is_success() {
            continue;
        }
        let Ok(narinfo) = response.text() else {
            continue;
        };
        let signed = narinfo
            .lines()
            .filter_map(|line| line.strip_prefix("Sig: "))
            .filter_map(|signature| signature.split_once(':'))
            .any(|(name, _)| names.contains(&name));
        if signed {
            return Ok(format!("{base} serves ScottyLabs packages"));
        }
    }
    anyhow::bail!(
        "no cache Nix uses serves ScottyLabs packages, so nix develop builds them from source"
    )
}

fn check_cache(checks: &mut Vec<Check>, offline: bool) {
    if offline {
        skipped(
            checks,
            "nix-cache",
            "offline mode; cache access was not checked",
        );
        return;
    }
    let Some(hash) = store_hash() else {
        skipped(
            checks,
            "nix-cache",
            "collar is not installed from the Nix store, so there is no ScottyLabs package to look up",
        );
        return;
    };
    record(
        checks,
        "nix-cache",
        binary_cache(&hash),
        "Add the ScottyLabs cache to the system Nix configuration and restart the Nix daemon: https://docs.kennel.scottylabs.org/guides/local-development/",
    );
}

fn check_revision(checks: &mut Vec<Check>, source: &Source, name: &str, action: &str) {
    record(
        checks,
        name,
        match &source.revision {
            Some(_) => Ok(source.describe()),
            None => checkout_revision(&source.directory),
        },
        action,
    );
}

fn check_governance(
    checks: &mut Vec<Check>,
    governance: Result<Source>,
    selected: &Result<Repo>,
) -> Option<project::Project> {
    let action = "Refresh the canonical governance snapshot online or provide --governance PATH.";
    let context = match (&governance, selected) {
        (Err(error), _) | (_, Err(error)) => {
            record(checks, "governance", Err(error), action);
            None
        }
        (Ok(source), Ok(repo)) => {
            let context = project::resolve(&source.directory, repo);
            record(
                checks,
                "governance",
                context
                    .as_ref()
                    .map(|project| format!("project {}, team {}", project.slug, project.team)),
                action,
            );
            context.ok()
        }
    };
    if let Ok(source) = governance {
        check_revision(
            checks,
            &source,
            "governance-revision",
            "Inspect local changes and fetch/rebase the governance checkout; doctor never updates it.",
        );
    }
    context
}

fn check_local(checks: &mut Vec<Check>, offline: bool) -> Result<PathBuf> {
    let root = repo::root();
    let processes = root.as_ref().is_ok_and(|root| {
        ["pitchfork.toml", ".config/pitchfork.toml"]
            .iter()
            .any(|path| root.join(path).symlink_metadata().is_ok())
    });
    let secrets = root
        .as_ref()
        .is_ok_and(|root| root.join("secretspec.toml").symlink_metadata().is_ok());
    for (tool, version, required) in [
        ("git", "--version", true),
        ("nix", "--version", true),
        ("pitchfork", "--version", processes),
        ("bao", "version", secrets),
        ("secretspec", "--version", secrets),
    ] {
        if !required {
            skipped(
                checks,
                &format!("tool:{tool}"),
                "no corresponding project configuration; enter nix develop to generate local process configuration",
            );
            continue;
        }
        record(
            checks,
            &format!("tool:{tool}"),
            process::check(Command::new(tool).arg(version), Duration::from_secs(5))
                .map(|()| "available".into()),
            "Install Lix and enter the project's dev shell with nix develop; ScottyLabs shared tooling supplies pitchfork, Bao, and secretspec.",
        );
    }
    check_cache(checks, offline);
    record(
        checks,
        "checkout",
        root.as_ref().map(|path| path.display().to_string()),
        "Run doctor inside a project Git checkout.",
    );
    if let Ok(root) = &root {
        for key in ["user.name", "user.email"] {
            record(
                checks,
                &format!("git:{key}"),
                git(root, &["config", "--get", key]).and_then(|value| {
                    ensure!(!value.is_empty(), "Git identity is empty");
                    Ok("configured".into())
                }),
                "Configure your Git author name and email with git config.",
            );
        }
        record(
            checks,
            "flake",
            flake(root),
            "Follow the Kennel local development guide: https://docs.kennel.scottylabs.org/guides/local-development/",
        );
    }
    record(
        checks,
        "ssh-agent",
        process::check(Command::new("ssh-add").arg("-l"), Duration::from_secs(5))
            .map(|()| "agent has keys; forge authentication is not verified".into()),
        "Load your SSH key with ssh-add; register its public key in your forge's SSH settings.",
    );
    root
}

fn check_remote(
    checks: &mut Vec<Check>,
    selected: Result<Repo>,
    config: &Config,
    offline: bool,
    secrets: bool,
) {
    if offline {
        for name in ["forge", "forge-auth", "bao-auth"] {
            skipped(checks, name, "offline mode; remote access was not checked");
        }
    } else {
        match selected.and_then(|repo| auth::api(repo.origin, config)) {
            Ok(api) => {
                #[derive(Deserialize)]
                struct Version {
                    version: String,
                }
                record(
                    checks,
                    "forge",
                    api.json::<Version>(Method::GET, &["version"], &[], None)
                        .map(|version| format!("Forgejo {} reachable", version.version)),
                    "Check the selected forge host and network connection.",
                );
                record(
                    checks,
                    "forge-auth",
                    api.require_token()
                        .and_then(|()| api.json::<pr::User>(Method::GET, &["user"], &[], None))
                        .map(|user| format!("authenticated as {}", user.login)),
                    "Set a host-bound COLLAR_TOKEN and run collar auth status.",
                );
            }
            Err(error) => record(
                checks,
                "forge",
                Err(error),
                "Fix repository selection and host-bound authentication configuration.",
            ),
        }
        if !secrets {
            skipped(checks, "bao-auth", "project has no secretspec.toml");
            return;
        }
        record(
            checks,
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
}

fn print_checks(checks: &[Check], output: &Output) -> Result<()> {
    let failed = checks
        .iter()
        .filter(|check| matches!(check.status, Status::Fail))
        .count();
    output.print(&checks, || {
        for check in checks {
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

pub fn run(
    args: Args,
    sources: &Sources<'_>,
    selection: Option<&str>,
    host: Option<&str>,
) -> Result<()> {
    let config = sources.config;
    args.output
        .validate_fields(&["name", "status", "detail", "action"])?;
    let mut checks = Vec::new();
    let root = check_local(&mut checks, args.offline);
    let secrets = root
        .as_ref()
        .is_ok_and(|root| root.join("secretspec.toml").symlink_metadata().is_ok());
    let selected = config.repo(selection, host);
    record(
        &mut checks,
        "repository",
        selected
            .as_ref()
            .map(|repo| format!("{}{}/{}", repo.origin, repo.owner, repo.name)),
        "Configure upstream/origin or select the canonical repository with -R.",
    );
    let context = check_governance(
        &mut checks,
        sources.load(Kind::Governance, args.governance, args.offline),
        &selected,
    );
    if !secrets {
        skipped(&mut checks, "secretspec", "project has no secretspec.toml");
    } else if let (Ok(root), Some(project)) = (&root, &context) {
        record(
            &mut checks,
            "secretspec",
            secretspec(root, project.secret_namespace.as_deref(), &args.profile),
            "Align secretspec.toml with governance, declare the selected profile, then run secretspec check --no-prompt explicitly.",
        );
    } else {
        skipped(
            &mut checks,
            "secretspec",
            "requires a local project checkout and resolved governance namespace",
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
                .unwrap_or_else(|| ACCOUNTS.into()),
        ),
    });
    check_remote(&mut checks, selected, config, args.offline, secrets);
    print_checks(&checks, &args.output)
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
