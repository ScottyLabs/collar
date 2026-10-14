use std::path::PathBuf;

use anyhow::{Context, Result, bail, ensure};
use clap::{Subcommand, ValueEnum};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::io::Write;

use crate::client::{DEFAULT_HOST, host_url};
use crate::output::Output;
use crate::repo::Repo;

#[derive(Clone, Copy, ValueEnum)]
pub enum Key {
    Host,
    Governance,
    Infrastructure,
}

#[derive(Subcommand)]
pub enum Command {
    /// Show saved nonsecret settings and their file location.
    Get {
        #[command(flatten)]
        output: Output,
    },
    /// Save a default host or canonical checkout path.
    Set { key: Key, value: String },
    /// Remove a saved default.
    Unset { key: Key },
}

#[derive(Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Settings {
    host: Option<String>,
    governance: Option<PathBuf>,
    infrastructure: Option<PathBuf>,
}

pub struct Config {
    path: Option<PathBuf>,
    settings: Settings,
}

impl Config {
    pub fn load(explicit: Option<PathBuf>, disabled: bool, creating: bool) -> Result<Self> {
        if disabled {
            return Ok(Self {
                path: None,
                settings: Settings::default(),
            });
        }
        let is_explicit = explicit.is_some();
        let path = match explicit {
            Some(path) => path,
            None => ProjectDirs::from("", "", "collar")
                .context("cannot find configuration directory; use --config PATH or --no-config")?
                .config_dir()
                .join("config.toml"),
        };
        let path = std::path::absolute(path).context("resolve configuration path")?;
        let settings = match std::fs::read_to_string(&path) {
            Ok(text) => toml::from_str::<Settings>(&text).map_err(ConfigDecodeError)?,
            Err(error)
                if error.kind() == std::io::ErrorKind::NotFound && (!is_explicit || creating) =>
            {
                Settings::default()
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("read configuration {}", path.display()));
            }
        };
        if let Some(host) = &settings.host {
            host_url(host).context("invalid configured host")?;
        }
        Ok(Self {
            path: Some(path),
            settings,
        })
    }

    pub fn host(&self) -> &str {
        self.settings.host.as_deref().unwrap_or(DEFAULT_HOST)
    }

    pub fn repo(&self, explicit: Option<&str>, host: Option<&str>) -> Result<Repo> {
        match explicit {
            Some(value) if value.split('/').count() == 2 => {
                Repo::parse(value, Some(host.unwrap_or(self.host())))
            }
            _ => Repo::resolve(explicit, host),
        }
    }

    pub fn checkout(&self, explicit: Option<PathBuf>, key: Key) -> Result<PathBuf> {
        if let Some(path) = explicit {
            return Ok(path);
        }
        let (name, saved) = match key {
            Key::Governance => ("governance", &self.settings.governance),
            Key::Infrastructure => ("infrastructure", &self.settings.infrastructure),
            Key::Host => bail!("host is not a checkout"),
        };
        let saved = saved.as_ref().with_context(|| {
            format!("no {name} checkout; use --checkout PATH or collar config set {name} PATH")
        })?;
        Ok(if saved.is_absolute() {
            saved.clone()
        } else {
            self.path
                .as_ref()
                .context("configuration disabled")?
                .parent()
                .context("configuration has no parent")?
                .join(saved)
        })
    }

    pub fn run(mut self, command: Command) -> Result<()> {
        match command {
            Command::Get { output } => {
                let value = serde_json::json!({"path": self.path, "host": self.settings.host, "governance": self.settings.governance, "infrastructure": self.settings.infrastructure});
                let human = serde_json::to_string_pretty(&value)?;
                output.print(&value, || println!("{human}"))
            }
            Command::Set { key, value } => {
                match key {
                    Key::Host => self.settings.host = Some(host_url(&value)?.to_string()),
                    Key::Governance | Key::Infrastructure => {
                        let path =
                            std::fs::canonicalize(&value).context("resolve checkout directory")?;
                        let required: &[&str] = match key {
                            Key::Governance => &["data/org.toml", "data/teams"],
                            _ => &["flake.nix", "modules/hosts"],
                        };
                        for entry in required {
                            ensure!(path.join(entry).exists(), "checkout is missing {entry}");
                        }
                        match key {
                            Key::Governance => self.settings.governance = Some(path),
                            _ => self.settings.infrastructure = Some(path),
                        }
                    }
                }
                self.save()
            }
            Command::Unset { key } => {
                match key {
                    Key::Host => self.settings.host = None,
                    Key::Governance => self.settings.governance = None,
                    Key::Infrastructure => self.settings.infrastructure = None,
                }
                self.save()
            }
        }
    }

    fn save(&self) -> Result<()> {
        let path = self
            .path
            .as_ref()
            .context("configuration disabled; remove --no-config to save settings")?;
        let parent = path.parent().context("configuration has no parent")?;
        std::fs::create_dir_all(parent).context("create configuration directory")?;
        let mut file =
            tempfile::NamedTempFile::new_in(parent).context("create temporary configuration")?;
        file.write_all(toml::to_string_pretty(&self.settings)?.as_bytes())
            .context("write configuration")?;
        file.as_file().sync_all().context("sync configuration")?;
        file.persist(path).context("replace configuration")?;
        println!("Saved {}", path.display());
        Ok(())
    }
}

// TOML diagnostics may contain credentials copied into an unsupported setting.
struct ConfigDecodeError(toml::de::Error);
impl std::fmt::Display for ConfigDecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid Collar configuration at byte range {:?}; expected only host, governance, and infrastructure settings",
            self.0.span()
        )
    }
}
impl std::fmt::Debug for ConfigDecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}
impl std::error::Error for ConfigDecodeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_repository_hosts_override_saved_default() {
        let config = Config {
            path: None,
            settings: Settings {
                host: Some("saved.example".into()),
                ..Settings::default()
            },
        };
        assert_eq!(
            config
                .repo(Some("owner/repo"), None)
                .unwrap()
                .origin
                .host_str(),
            Some("saved.example")
        );
        assert_eq!(
            config
                .repo(Some("other.example/owner/repo"), None)
                .unwrap()
                .origin
                .host_str(),
            Some("other.example")
        );
        assert_eq!(
            config
                .repo(Some("owner/repo"), Some("flag.example"))
                .unwrap()
                .origin
                .host_str(),
            Some("flag.example")
        );
        assert!(
            config
                .repo(Some("other.example/owner/repo"), Some("flag.example"))
                .is_err()
        );
        assert_eq!(
            config
                .checkout(Some("flag-path".into()), Key::Governance)
                .unwrap(),
            PathBuf::from("flag-path")
        );
    }
}
