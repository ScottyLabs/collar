//! Keycloak sessions and the fixed-provider Forgejo broker protocol.

use std::io::Read;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, ensure};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header, jwk::JwkSet};
use reqwest::blocking::{Client, Response};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use url::Url;

pub mod browser;
pub mod logs;
mod oidc;
pub mod relay;

pub const CLIENT_ID: &str = "collar";
pub const ISSUER: &str = "https://idp.scottylabs.org/realms/scottylabs";
pub const BROKER: &str = "https://collar-broker.scottylabs.org/";
pub const GRAFANA: &str = "https://grafana.scottylabs.org/";
const LIMIT: u64 = 8 * 1024 * 1024;

#[derive(Clone)]
pub struct Endpoints {
    pub issuer: Url,
    pub broker: Url,
    pub grafana: Url,
}

impl Endpoints {
    pub fn from_env() -> Result<Self> {
        fn endpoint(name: &str, default: &str) -> Result<Url> {
            let value = match std::env::var(name) {
                Ok(value) => value,
                Err(std::env::VarError::NotPresent) => default.into(),
                Err(_) => anyhow::bail!("{name} is not UTF-8"),
            };
            secure_url(&value)
        }
        let endpoints = Self {
            issuer: endpoint("COLLAR_SSO_ISSUER", ISSUER)?,
            broker: endpoint("COLLAR_SSO_BROKER", BROKER)?,
            grafana: endpoint("COLLAR_GRAFANA", GRAFANA)?,
        };
        ensure!(
            endpoints.broker.path() == "/" && endpoints.grafana.path() == "/",
            "broker and Grafana must be origins"
        );
        Ok(endpoints)
    }

    pub fn oidc(&self, operation: &str) -> Result<Url> {
        Ok(Url::parse(&format!(
            "{}/protocol/openid-connect/{operation}",
            self.issuer.as_str().trim_end_matches('/')
        ))?)
    }

    pub fn matches(&self, session: &Session) -> bool {
        session.issuer == self.issuer.as_str()
            && session.client_id == CLIENT_ID
            && session.broker == self.broker.as_str()
            && session.grafana == self.grafana.as_str()
    }
}

pub fn secure_url(value: &str) -> Result<Url> {
    let url = Url::parse(value).context("invalid SSO endpoint")?;
    let loopback = url.host_str().is_some_and(|host| {
        host == "localhost"
            || host
                .trim_matches(['[', ']'])
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
    });
    ensure!(
        url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none()
            && (url.scheme() == "https" || (url.scheme() == "http" && loopback)),
        "SSO endpoints require HTTPS without credentials or query parameters"
    );
    Ok(url)
}

pub fn client() -> Result<Client> {
    Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()
        .context("create SSO HTTP client")
}

pub fn read_json<T: DeserializeOwned>(response: Response) -> Result<T> {
    let mut bytes = Vec::new();
    response
        .take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .context("read SSO response")?;
    ensure!(bytes.len() as u64 <= LIMIT, "SSO response exceeds 8 MiB");
    serde_json::from_slice(&bytes).map_err(|_| anyhow::anyhow!("invalid SSO response"))
}

pub fn now() -> Result<u64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before Unix epoch")?
        .as_secs())
}

pub fn secret(value: &str) -> Result<()> {
    ensure!(
        !value.trim().is_empty() && !value.contains(['\r', '\n']) && value.len() <= 65536,
        "invalid authentication credential"
    );
    Ok(())
}

#[derive(Deserialize)]
pub struct Claims {
    pub sub: String,
    pub azp: Option<String>,
}

pub fn verify(token: &str, issuer: &Url, audience: &str, keys: &JwkSet) -> Result<Claims> {
    let header = decode_header(token).map_err(|_| anyhow::anyhow!("invalid identity token"))?;
    ensure!(
        header.alg == Algorithm::RS256,
        "unsupported identity signing algorithm"
    );
    let key = keys
        .find(
            header
                .kid
                .as_deref()
                .context("identity token has no signing key")?,
        )
        .context("unknown identity signing key")?;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[issuer.as_str()]);
    validation.set_audience(&[audience]);
    validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
    validation.leeway = 0;
    validation.validate_nbf = true;
    let claims = decode::<Claims>(
        token,
        &DecodingKey::from_jwk(key).context("invalid identity signing key")?,
        &validation,
    )
    .map_err(|_| anyhow::anyhow!("identity token verification failed"))?
    .claims;
    ensure!(
        !claims.sub.is_empty() && claims.azp.as_deref() == Some(CLIENT_ID),
        "identity token is not issued to Collar"
    );
    Ok(claims)
}

pub fn keys(endpoints: &Endpoints) -> Result<JwkSet> {
    let response = client()?
        .get(endpoints.oidc("certs")?)
        .send()
        .map_err(|_| anyhow::anyhow!("identity signing keys unavailable"))?;
    ensure!(
        response.status().is_success(),
        "identity signing keys unavailable"
    );
    read_json(response)
}

#[derive(Serialize, Deserialize)]
pub struct Session {
    pub issuer: String,
    pub client_id: String,
    pub broker: String,
    pub grafana: String,
    pub subject: String,
    pub nonce: String,
    pub auth_time: Option<u64>,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: u64,
}

pub fn refresh(endpoints: &Endpoints, session: &Session) -> Result<Session> {
    ensure!(
        endpoints.matches(session),
        "saved SSO session belongs to different endpoints; sign in again"
    );
    oidc::Oidc::discover(endpoints)?.refresh(endpoints, session)
}

pub fn logout(endpoints: &Endpoints, session: &Session) -> Result<()> {
    ensure!(
        endpoints.matches(session),
        "saved SSO session belongs to different endpoints"
    );
    let body = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs([
            ("client_id", CLIENT_ID),
            ("refresh_token", session.refresh_token.as_str()),
        ])
        .finish();
    let response = client()?
        .post(endpoints.oidc("logout")?)
        .header("content-type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .map_err(|_| anyhow::anyhow!("SSO logout could not be confirmed"))?;
    ensure!(response.status().is_success(), "SSO logout rejected");
    Ok(())
}

#[derive(Serialize, Deserialize)]
pub struct ForgeToken {
    pub access_token: String,
    pub expires_in: u64,
}

pub fn forge(endpoints: &Endpoints, session: &Session) -> Result<Option<ForgeToken>> {
    ensure!(
        endpoints.matches(session),
        "saved SSO session belongs to different endpoints"
    );
    let response = client()?
        .post(endpoints.broker.join("token")?)
        .bearer_auth(&session.access_token)
        .send()
        .map_err(|_| anyhow::anyhow!("Forgejo broker unavailable"))?;
    if response.status().as_u16() == 409 {
        return Ok(None);
    }
    ensure!(
        response.status().is_success(),
        "Forgejo broker rejected the session; sign in again"
    );
    let token: ForgeToken = read_json(response)?;
    secret(&token.access_token)?;
    ensure!(
        token.expires_in > 0,
        "Forgejo broker returned an expired token"
    );
    Ok(Some(token))
}

pub fn grafana(endpoints: &Endpoints, session: &Session) -> Result<serde_json::Value> {
    ensure!(
        endpoints.matches(session),
        "saved SSO session belongs to different endpoints"
    );
    let response = client()?
        .get(endpoints.grafana.join("api/user")?)
        .header("X-JWT-Assertion", &session.access_token)
        .send()
        .map_err(|_| anyhow::anyhow!("Grafana authentication unavailable"))?;
    ensure!(
        response.status().is_success(),
        "Grafana rejected the SSO session"
    );
    read_json(response)
}
