use std::io::Read;

use anyhow::{Context, Result, anyhow, ensure};
use reqwest::Method;
use url::Url;

use crate::client::{Api, host_url};
use crate::config::Config;

const SESSION_PREFIX: &str = "collar-sso-v1:";

trait CredentialStore {
    fn get(&self, origin: &Url) -> Result<Option<String>>;
    fn set(&self, origin: &Url, token: &str) -> Result<()>;
    fn delete(&self, origin: &Url) -> Result<()>;
}

struct OsStore;

fn store_error(error: keyring::Error) -> anyhow::Error {
    if matches!(error, keyring::Error::Ambiguous(_)) {
        return anyhow!(
            "multiple OS credentials match this forge host; remove duplicate Collar entries from your credential manager"
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
    let Some(token) = store.get(origin)? else {
        return Ok(None);
    };
    ensure!(
        !token.starts_with("collar-oauth-v1:"),
        "legacy Forgejo OAuth session; run collar auth login --web to replace it with SSO"
    );
    validate_secret(&token, "saved forge credential")?;
    Ok(Some(token))
}

fn validate_secret(value: &str, label: &str) -> Result<()> {
    ensure!(
        !value.trim().is_empty() && !value.contains(['\r', '\n']),
        "{label} is invalid"
    );
    Ok(())
}

struct SsoStore(collar::sso::Endpoints);
impl SsoStore {
    fn entry(&self, origin: &Url) -> Result<keyring::Entry> {
        keyring::Entry::new(
            "collar.sso",
            &format!("{origin}|{}|{}", self.0.issuer, collar::sso::CLIENT_ID),
        )
        .map_err(store_error)
    }
}
impl CredentialStore for SsoStore {
    fn get(&self, origin: &Url) -> Result<Option<String>> {
        match self.entry(origin)?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(store_error(error)),
        }
    }
    fn set(&self, origin: &Url, value: &str) -> Result<()> {
        self.entry(origin)?.set_password(value).map_err(store_error)
    }
    fn delete(&self, origin: &Url) -> Result<()> {
        match self.entry(origin)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(store_error(error)),
        }
    }
}

fn session(
    origin: &Url,
    config: &Config,
    store: &impl CredentialStore,
    endpoints: &collar::sso::Endpoints,
) -> Result<Option<collar::sso::Session>> {
    if !config.has_login(origin) {
        return Ok(None);
    }
    let Some(value) = store.get(origin)? else {
        return Ok(None);
    };
    let json = value
        .strip_prefix(SESSION_PREFIX)
        .context("invalid saved SSO session; sign in again")?;
    let mut session: collar::sso::Session = serde_json::from_str(json)
        .map_err(|_| anyhow!("invalid saved SSO session; sign in again"))?;
    ensure!(
        endpoints.matches(&session),
        "saved SSO session belongs to different endpoints; sign in again"
    );
    if session.expires_at <= collar::sso::now()?.saturating_add(30) {
        session = collar::sso::refresh(endpoints, &session)?;
        // Save refresh rotation before any subsequent service call can fail.
        store.set(origin, &encode_session(&session)?)?;
    }
    Ok(Some(session))
}

fn encode_session(session: &collar::sso::Session) -> Result<String> {
    Ok(format!(
        "{SESSION_PREFIX}{}",
        serde_json::to_string(session).context("encode SSO session")?
    ))
}

pub fn grafana_session(
    origin: &Url,
    config: &Config,
) -> Result<(collar::sso::Endpoints, collar::sso::Session)> {
    let endpoints = collar::sso::Endpoints::from_env()?;
    let value = session(origin, config, &SsoStore(endpoints.clone()), &endpoints)?
        .context("Grafana requires collar auth login through SSO")?;
    Ok((endpoints, value))
}

fn environment_token(origin: &Url, config: &Config) -> Result<Option<String>> {
    let configured = match std::env::var("COLLAR_HOST") {
        Ok(host) => host,
        Err(std::env::VarError::NotPresent) => config.host().into(),
        Err(error) => return Err(error).context("read COLLAR_HOST"),
    };
    let environment = if *origin == host_url(&configured)? {
        match std::env::var("COLLAR_TOKEN") {
            Ok(token) => Some(token),
            Err(std::env::VarError::NotPresent) => None,
            Err(error) => return Err(error).context("read COLLAR_TOKEN"),
        }
    } else {
        None
    };
    Ok(environment)
}

pub fn api(origin: Url, config: &Config) -> Result<Api> {
    let environment = environment_token(&origin, config)?;
    let token = match environment {
        Some(token) => Some(token),
        None => match saved_token(&origin, config, &OsStore)? {
            Some(token) => Some(token),
            None if config.has_login(&origin) => {
                let endpoints = collar::sso::Endpoints::from_env()?;
                let session = session(&origin, config, &SsoStore(endpoints.clone()), &endpoints)?
                    .context("saved SSO session is missing; run collar auth login")?;
                Some(
                    collar::sso::forge(&endpoints, &session)?
                        .context("Forgejo account link required; run collar auth login")?
                        .access_token,
                )
            }
            None => None,
        },
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
        ensure!(
            origin.as_str() == "https://git.cmu.dev/",
            "university SSO is bound to git.cmu.dev; use an explicit token for other forge hosts"
        );
        let endpoints = collar::sso::Endpoints::from_env()?;
        let store = SsoStore(endpoints.clone());
        let previous = if web {
            None
        } else {
            session(&origin, config, &store, &endpoints)?
        };
        let mut session = match previous {
            Some(session) => session,
            None => collar::sso::browser::login(&endpoints, false)?,
        };
        persist(config, &store, &origin, Some(&encode_session(&session)?))?;
        OsStore.delete(&origin)?;
        let mut forge = collar::sso::forge(&endpoints, &session)?;
        if forge.is_none() {
            let linked = collar::sso::browser::login(&endpoints, true)?;
            ensure!(
                linked.subject == session.subject,
                "account linking changed the signed-in user"
            );
            session = linked;
            persist(config, &store, &origin, Some(&encode_session(&session)?))?;
            forge = collar::sso::forge(&endpoints, &session)?;
        }
        let forge = forge.context(
            "Forgejo account is still not linked; complete account linking and sign in again",
        )?;
        let api = Api::new(origin.clone(), Some(forge.access_token))?;
        let user: crate::pr::User = api.json(Method::GET, &["user"], &[], None)?;
        ensure!(
            !user.login.trim().is_empty() && !user.login.chars().any(char::is_control),
            "forge returned an invalid login name"
        );
        println!(
            "Logged in to {origin} as {} through university SSO",
            user.login
        );
        collar::sso::grafana(&endpoints, &session)?;
        println!("Grafana access verified at {}", endpoints.grafana);
        return Ok(());
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
    config.require_writable()?;
    let endpoints = collar::sso::Endpoints::from_env()?;
    let store = SsoStore(endpoints.clone());
    logout_with(config, origin, &endpoints, &store, &OsStore)
}

fn logout_with(
    config: &mut Config,
    origin: Url,
    endpoints: &collar::sso::Endpoints,
    store: &impl CredentialStore,
    forge_store: &impl CredentialStore,
) -> Result<()> {
    let revoked = match store.get(&origin)? {
        Some(value) => {
            let decoded = value
                .strip_prefix(SESSION_PREFIX)
                .and_then(|value| serde_json::from_str::<collar::sso::Session>(value).ok());
            match decoded {
                Some(session) => collar::sso::logout(endpoints, &session),
                None => Err(anyhow!(
                    "invalid saved SSO session; remote logout cannot be confirmed"
                )),
            }
        }
        None => Ok(()),
    };
    let removed = store.delete(&origin);
    let metadata = persist(config, forge_store, &origin, None);
    removed?;
    metadata?;
    revoked.context("local credentials removed; remote SSO logout was not confirmed; revoke the session in Keycloak")?;
    println!("Removed saved credentials for {origin}; its SSO session was revoked when present");
    Ok(())
}

pub fn status(config: &Config, origin: Url) -> Result<()> {
    let api = api(origin.clone(), config)?;
    api.require_token()?;
    let user: crate::pr::User = api.json(Method::GET, &["user"], &[], None)?;
    println!("Logged in to {origin} as {}", user.login);
    if environment_token(&origin, config)?.is_some()
        || !config.has_login(&origin)
        || OsStore.get(&origin)?.is_some()
    {
        return Ok(());
    }
    let endpoints = collar::sso::Endpoints::from_env()?;
    if let Some(session) = session(&origin, config, &SsoStore(endpoints.clone()), &endpoints)? {
        collar::sso::grafana(&endpoints, &session)?;
        println!("Grafana access verified at {}", endpoints.grafana);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
