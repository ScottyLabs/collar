use std::path::PathBuf;

use anyhow::{Context, Result};
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
}

#[derive(Subcommand)]
pub enum Command {
    /// Show saved nonsecret settings and their file location.
    Get {
        #[command(flatten)]
        output: Output,
    },
    /// Save the default forge host.
    Set { key: Key, value: String },
    /// Remove a saved default.
    Unset { key: Key },
}

#[derive(Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Settings {
    host: Option<String>,
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
            Ok(text) => crate::document::parse(&text, "Collar configuration")
                .context("expected only a host setting")?,
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

    pub fn run(mut self, command: Command) -> Result<()> {
        match command {
            Command::Get { output } => {
                let value = serde_json::json!({"path": self.path, "host": self.settings.host});
                let human = serde_json::to_string_pretty(&value)?;
                output.print(&value, || println!("{human}"))
            }
            Command::Set { key, value } => {
                match key {
                    Key::Host => self.settings.host = Some(host_url(&value)?.to_string()),
                }
                self.save()
            }
            Command::Unset { key } => {
                match key {
                    Key::Host => self.settings.host = None,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_repository_hosts_override_saved_default() {
        let config = Config {
            path: None,
            settings: Settings {
                host: Some("saved.example".into()),
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
    }
}
