use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use serde::Serialize;

#[derive(Serialize)]
pub struct Host {
    pub name: String,
    pub configuration: PathBuf,
    pub role: PathBuf,
}

pub fn hosts(checkout: &Path) -> Result<Vec<Host>> {
    let directory = checkout.join("modules/hosts");
    let mut hosts = Vec::new();
    for entry in std::fs::read_dir(&directory)
        .with_context(|| format!("read infrastructure hosts at {}", directory.display()))?
    {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| anyhow::anyhow!("host directory name is not UTF-8"))?;
        let configuration = entry.path().join("configuration.nix");
        let role = checkout.join("modules/roles").join(format!("{name}.nix"));
        ensure!(
            configuration.is_file() && role.is_file(),
            "host {name} is missing configuration.nix or its role module"
        );
        hosts.push(Host {
            name,
            configuration,
            role,
        });
    }
    hosts.sort_by(|a, b| a.name.cmp(&b.name));
    ensure!(
        !hosts.is_empty(),
        "infrastructure checkout has no host directories"
    );
    Ok(hosts)
}

#[derive(Serialize)]
pub struct Service {
    pub host: String,
    pub source: PathBuf,
}

pub fn service(checkout: &Path, name: &str) -> Result<Vec<Service>> {
    ensure!(
        !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_')),
        "service name must contain only letters, digits, hyphens, or underscores"
    );
    let mut matches = Vec::new();
    for host in hosts(checkout)? {
        let directory = host
            .configuration
            .parent()
            .context("host configuration has no parent")?;
        for source in [
            directory.join(format!("{name}.nix")),
            directory.join(name).join("default.nix"),
        ] {
            if source.is_file() {
                matches.push(Service {
                    host: host.name.clone(),
                    source,
                });
            }
        }
    }
    ensure!(
        !matches.is_empty(),
        "no host configuration file found for service {name}"
    );
    Ok(matches)
}
