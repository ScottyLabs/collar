use std::io::Read;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail, ensure};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::client::{Api, host_url};
use crate::config::Config;

const OAUTH_BUNDLE_PREFIX: &str = "collar-oauth-v1:";

#[derive(Serialize, Deserialize)]
struct OAuthBundle {
    access_token: String,
    refresh_token: Option<String>,
    expires_at: Option<u64>,
}

enum StoredCredential {
    Token(String),
    OAuth(OAuthBundle),
}

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
    let stored = store
        .get(origin)?
        .context("saved forge credential is missing; run collar auth login again")?;
    match decode_credential(&stored)? {
        StoredCredential::Token(token) => Ok(Some(token)),
        StoredCredential::OAuth(bundle) => {
            if let Some(expires_at) = bundle.expires_at
                && expires_at <= unix_time()?
            {
                bail!("saved OAuth access token expired; run collar auth login to refresh it");
            }
            Ok(Some(bundle.access_token))
        }
    }
}

fn saved_refresh_token(
    origin: &Url,
    config: &Config,
    store: &impl CredentialStore,
) -> Result<Option<String>> {
    if !config.has_login(origin) {
        return Ok(None);
    }
    let stored = store
        .get(origin)?
        .context("saved forge credential is missing; run collar auth login again")?;
    match decode_credential(&stored)? {
        StoredCredential::Token(_) => Ok(None),
        StoredCredential::OAuth(bundle) => Ok(bundle.refresh_token),
    }
}

fn decode_credential(stored: &str) -> Result<StoredCredential> {
    let Some(encoded) = stored.strip_prefix(OAUTH_BUNDLE_PREFIX) else {
        ensure!(
            !stored.trim().is_empty() && !stored.contains(['\r', '\n']),
            "saved forge credential is invalid; run collar auth login again"
        );
        return Ok(StoredCredential::Token(stored.to_owned()));
    };
    let bundle: OAuthBundle = serde_json::from_str(encoded)
        .context("saved OAuth credential is invalid; run collar auth login again")?;
    validate_secret(&bundle.access_token, "saved OAuth access token")?;
    if let Some(refresh_token) = &bundle.refresh_token {
        validate_secret(refresh_token, "saved OAuth refresh token")?;
    }
    Ok(StoredCredential::OAuth(bundle))
}

fn encode_oauth(bundle: &OAuthBundle) -> Result<String> {
    validate_secret(&bundle.access_token, "OAuth access token")?;
    if let Some(refresh_token) = &bundle.refresh_token {
        validate_secret(refresh_token, "OAuth refresh token")?;
    }
    Ok(format!(
        "{OAUTH_BUNDLE_PREFIX}{}",
        serde_json::to_string(bundle).context("encode OAuth credential bundle")?
    ))
}

fn validate_secret(value: &str, label: &str) -> Result<()> {
    ensure!(
        !value.trim().is_empty() && !value.contains(['\r', '\n']),
        "{label} is invalid"
    );
    Ok(())
}

fn unix_time() -> Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before Unix epoch")
        .map(|duration| duration.as_secs())
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

pub fn login(
    config: &mut Config,
    origin: Url,
    with_token: bool,
    token_prompt: bool,
    web: bool,
) -> Result<()> {
    config.require_writable()?;
    ensure!(
        !(with_token && token_prompt),
        "--with-token and --token cannot be used together"
    );
    let (token, stored_credential) = if with_token {
        let mut token = String::new();
        std::io::stdin()
            .take(65537)
            .read_to_string(&mut token)
            .context("read token from stdin")?;
        ensure!(token.len() <= 65536, "token input exceeds 64 KiB");
        let token = token.trim_end_matches(['\r', '\n']).to_owned();
        (token.clone(), token)
    } else if token_prompt {
        crate::tui::require_terminal()
            .context("--token requires a terminal; use --with-token to read a token from stdin")?;
        let terminal = console::Term::stderr();
        terminal.write_str(&format!("Personal access token for {origin}: "))?;
        let token = terminal
            .read_secure_line()
            .context("read token without echo")?;
        (token.clone(), token)
    } else {
        if !web {
            crate::tui::require_terminal().context(
                "browser sign-in requires a terminal; pass --web to explicitly start it or use --with-token for token input",
            )?;
        }
        let previous_refresh = saved_refresh_token(&origin, config, &OsStore)?;
        let mut credential = crate::oauth::browser_login(&origin, previous_refresh.as_deref())?;
        if credential.refresh_token.is_none() {
            credential.refresh_token = previous_refresh;
        }
        let token = credential.access_token.clone();
        let stored = encode_oauth(&OAuthBundle {
            access_token: credential.access_token,
            refresh_token: credential.refresh_token,
            expires_at: credential.expires_at,
        })?;
        (token, stored)
    };
    let api = Api::new(origin.clone(), Some(token.clone()))?;
    let user: crate::pr::User = api.json(Method::GET, &["user"], &[], None)?;
    ensure!(
        !user.login.trim().is_empty() && !user.login.chars().any(char::is_control),
        "forge returned an invalid login name"
    );
    persist(config, &OsStore, &origin, Some(&stored_credential))?;
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
mod tests;
