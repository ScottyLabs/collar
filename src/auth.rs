use std::io::Read;

use anyhow::{Context, Result, anyhow, ensure};
use reqwest::Method;
use url::Url;

use crate::client::{Api, host_url};
use crate::config::Config;

trait CredentialStore {
    fn get(&self, origin: &Url) -> Result<Option<String>>;
    fn set(&self, origin: &Url, token: &str) -> Result<()>;
    fn delete(&self, origin: &Url) -> Result<()>;
}

struct OsStore;

fn store_error(error: keyring::Error) -> anyhow::Error {
    if matches!(error, keyring::Error::Ambiguous(_)) {
        return anyhow!(
            "multiple OS credentials match this forge host; remove duplicate collar.forge entries from your credential manager"
        );
    }
    anyhow!(
        "OS credential store operation failed; unlock your credential store (Linux also needs a session bus), or use host-bound COLLAR_TOKEN for headless access"
    )
}

fn entry(origin: &Url) -> Result<keyring::Entry> {
    keyring::Entry::new("collar.forge", origin.as_str()).map_err(store_error)
}

impl CredentialStore for OsStore {
    fn get(&self, origin: &Url) -> Result<Option<String>> {
        match entry(origin)?.get_password() {
            Ok(token) => Ok(Some(token)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(store_error(error)),
        }
    }
    fn set(&self, origin: &Url, token: &str) -> Result<()> {
        entry(origin)?.set_password(token).map_err(store_error)
    }
    fn delete(&self, origin: &Url) -> Result<()> {
        match entry(origin)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(store_error(error)),
        }
    }
}

fn saved_token(
    origin: &Url,
    config: &Config,
    store: &impl CredentialStore,
) -> Result<Option<String>> {
    if !config.has_login(origin) {
        return Ok(None);
    }
    let token = store
        .get(origin)?
        .context("saved forge credential is missing; run collar auth login again")?;
    Ok(Some(token))
}

pub fn api(origin: Url, config: &Config) -> Result<Api> {
    let configured = match std::env::var("COLLAR_HOST") {
        Ok(host) => host,
        Err(std::env::VarError::NotPresent) => config.host().into(),
        Err(error) => return Err(error).context("read COLLAR_HOST"),
    };
    let environment = if origin == host_url(&configured)? {
        match std::env::var("COLLAR_TOKEN") {
            Ok(token) => Some(token),
            Err(std::env::VarError::NotPresent) => None,
            Err(error) => return Err(error).context("read COLLAR_TOKEN"),
        }
    } else {
        None
    };
    let token = match environment {
        Some(token) => Some(token),
        None => saved_token(&origin, config, &OsStore)?,
    };
    Api::new(origin, token)
}

fn persist(
    config: &mut Config,
    store: &impl CredentialStore,
    origin: &Url,
    token: Option<&str>,
) -> Result<()> {
    config.require_writable()?;
    let previous = store.get(origin)?;
    match token {
        Some(token) => store.set(origin, token)?,
        None => store.delete(origin)?,
    }
    if let Err(error) = config.set_login(origin, token.is_some()) {
        let error = error.context("could not save login metadata");
        let rollback = match previous {
            Some(token) => store.set(origin, &token),
            None => store.delete(origin),
        };
        return match rollback {
            Ok(()) => Err(error.context("previous credential restored")),
            Err(rollback) => Err(error.context(format!("credential rollback also failed: {rollback}; run auth logout for this host after unlocking the store"))),
        };
    }
    Ok(())
}

pub fn login(config: &mut Config, origin: Url, with_token: bool) -> Result<()> {
    config.require_writable()?;
    let token = if with_token {
        let mut token = String::new();
        std::io::stdin()
            .take(65537)
            .read_to_string(&mut token)
            .context("read token from stdin")?;
        ensure!(token.len() <= 65536, "token input exceeds 64 KiB");
        token.trim_end_matches(['\r', '\n']).to_owned()
    } else {
        crate::tui::require_terminal()
            .context("use --with-token to explicitly read a token from stdin")?;
        let terminal = console::Term::stderr();
        terminal.write_str(&format!("Personal access token for {origin}: "))?;
        terminal
            .read_secure_line()
            .context("read token without echo")?
    };
    let api = Api::new(origin.clone(), Some(token.clone()))?;
    let user: crate::pr::User = api.json(Method::GET, &["user"], &[], None)?;
    ensure!(
        !user.login.trim().is_empty() && !user.login.chars().any(char::is_control),
        "forge returned an invalid login name"
    );
    persist(config, &OsStore, &origin, Some(&token))?;
    println!(
        "Logged in to {origin} as {}; credential saved in the OS store",
        user.login
    );
    Ok(())
}

pub fn logout(config: &mut Config, origin: Url) -> Result<()> {
    persist(config, &OsStore, &origin, None)?;
    println!(
        "Removed the saved credential for {origin}; environment tokens and server tokens are unchanged"
    );
    Ok(())
}

#[cfg(test)]
#[path = "auth_tests.rs"]
mod tests;
