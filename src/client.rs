use std::io::Read;
use std::time::Duration;
use std::{error::Error, fmt};

use anyhow::{Context, Result, bail, ensure};
use reqwest::Method;
use reqwest::blocking::{Client, Response};
use serde::de::DeserializeOwned;
use serde_json::Value;
use url::Url;

mod cache;

pub const DEFAULT_HOST: &str = "git.cmu.dev";
const BODY_LIMIT: u64 = 8 * 1024 * 1024;
const PAGE_LIMIT: usize = 100;

pub fn host_url(host: &str) -> Result<Url> {
    let url = Url::parse(&if host.contains("://") {
        host.to_owned()
    } else {
        format!("https://{host}")
    })
    .context("invalid forge host")?;
    ensure!(
        url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.path() == "/"
            && url.query().is_none()
            && url.fragment().is_none(),
        "host must be an origin without credentials, a path, or a query"
    );
    let loopback = url.host_str().is_some_and(|host| {
        host == "localhost"
            || host
                .trim_matches(['[', ']'])
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
    });
    ensure!(
        url.scheme() == "https" || (url.scheme() == "http" && loopback),
        "forge connections require HTTPS (HTTP is allowed only on loopback)"
    );
    Ok(url)
}

fn http_client() -> Result<Client> {
    Client::builder()
        .user_agent(concat!("collar/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .build()
        .context("create HTTP client")
}

fn read_body(response: Response) -> Result<String> {
    let mut bytes = Vec::new();
    response
        .take(BODY_LIMIT + 1)
        .read_to_end(&mut bytes)
        .context("read response")?;
    ensure!(bytes.len() as u64 <= BODY_LIMIT, "response exceeds 8 MiB");

    String::from_utf8(bytes).context("response is not UTF-8")
}

/// Fetches public JSON outside the forge API without sending the forge token
pub fn public_json<T: DeserializeOwned>(url: Url) -> Result<T> {
    let path = url.path().to_owned();
    let response = http_client()?
        .get(url)
        .send()
        .map_err(|error| anyhow::anyhow!("GET {path} failed: {}", error.without_url()))?;
    let status = response.status();
    ensure!(status.is_success(), "GET {path}: HTTP {}", status.as_u16());

    let text = read_body(response)?;
    serde_json::from_str(&text).map_err(|error| {
        anyhow::anyhow!(
            "GET {path} returned invalid JSON at line {}, column {}",
            error.line(),
            error.column()
        )
    })
}

#[derive(Clone, Default)]
pub struct ReadCache {
    pub directory: Option<std::path::PathBuf>,
    pub refresh: bool,
}
pub struct Api {
    client: Client,
    origin: Url,
    token: Option<String>,
    cache: ReadCache,
}

impl Api {
    pub fn new(origin: Url, token: Option<String>) -> Result<Self> {
        let origin = host_url(origin.as_str())?;
        if let Some(token) = &token {
            ensure!(
                !token.trim().is_empty() && !token.contains(['\r', '\n']),
                "forge token must be nonempty and contain no newlines"
            );
        }
        Ok(Self {
            client: http_client()?,
            origin,
            token,
            cache: ReadCache::default(),
        })
    }

    pub fn with_read_cache(mut self, cache: ReadCache) -> Self {
        self.cache = cache;
        self
    }

    pub fn origin(&self) -> &Url {
        &self.origin
    }

    pub fn require_token(&self) -> Result<()> {
        ensure!(
            self.token.is_some(),
            "this operation requires collar auth login or COLLAR_TOKEN; bind environment tokens with COLLAR_HOST"
        );
        Ok(())
    }

    pub fn text(
        &self,
        method: Method,
        path: &[&str],
        query: &[(String, String)],
        body: Option<&Value>,
    ) -> Result<String> {
        if method == Method::GET && body.is_none() {
            return self.read_text(path, query);
        }
        read_body(self.request(method, path, query, body)?)
    }

    fn request(
        &self,
        method: Method,
        path: &[&str],
        query: &[(String, String)],
        body: Option<&Value>,
    ) -> Result<Response> {
        let mutating = !matches!(method, Method::GET | Method::HEAD);
        if mutating {
            self.invalidate_read_cache();
        }
        let result = self.request_headers(method.clone(), path, query, body, &[]);
        if mutating {
            self.invalidate_read_cache();
        }
        result
    }
    fn request_headers(
        &self,
        method: Method,
        path: &[&str],
        query: &[(String, String)],
        body: Option<&Value>,
        headers: &[(&str, &str)],
    ) -> Result<Response> {
        if !matches!(method, Method::GET | Method::HEAD) {
            self.require_token()?;
        }
        ensure!(
            path.iter()
                .all(|part| !part.is_empty() && *part != "." && *part != ".."),
            "API path must not contain empty or traversal segments"
        );
        let mut url = self.origin.clone();
        url.path_segments_mut()
            .map_err(|()| anyhow::anyhow!("forge host cannot contain API paths"))?
            .extend(["api", "v1"])
            .extend(path);
        if !query.is_empty() {
            url.query_pairs_mut().extend_pairs(query);
        }
        let mut request = self.client.request(method.clone(), url);
        if let Some(token) = &self.token {
            request = request.header("Authorization", format!("token {token}"));
        }
        if let Some(body) = body {
            request = request.json(body);
        }
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let response = request
            .send()
            .map_err(|error| anyhow::anyhow!("forge request failed: {}", error.without_url()))?;
        let status = response.status();
        if !status.is_success()
            && !(status == reqwest::StatusCode::NOT_MODIFIED && !headers.is_empty())
        {
            let advice = match status.as_u16() {
                401 | 403 => "check COLLAR_TOKEN and its permissions",
                404 => "the resource or API capability is unavailable on this host",
                409 | 422 => "the server rejected the change; inspect the resource before retrying",
                429 => "rate limited; try again later",
                300..=399 => "redirect refused; use the canonical forge host",
                _ => "the server could not complete the request",
            };
            return Err(HttpStatus {
                status: status.as_u16(),
                message: format!(
                    "{method} {}: HTTP {}: {advice}",
                    path.join("/"),
                    status.as_u16()
                ),
            }
            .into());
        }
        Ok(response)
    }

    pub fn json<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &[&str],
        query: &[(String, String)],
        body: Option<&Value>,
    ) -> Result<T> {
        let text = self.text(method, path, query, body)?;
        serde_json::from_str(&text).map_err(|error| ResponseDecodeError(error).into())
    }

    pub fn list<T: DeserializeOwned>(
        &self,
        path: &[&str],
        query: &[(String, String)],
        limit: usize,
    ) -> Result<Vec<T>> {
        ensure!(limit > 0, "list limit must be positive");
        let mut items = Vec::new();
        for page in 1..=PAGE_LIMIT {
            let mut query = query.to_vec();
            query.extend([
                ("page".into(), page.to_string()),
                ("limit".into(), "50".into()),
            ]);
            let batch: Vec<T> = self.json(Method::GET, path, &query, None)?;
            if batch.is_empty() {
                return Ok(items);
            }
            items.extend(batch);
            if items.len() >= limit {
                items.truncate(limit);
                return Ok(items);
            }
        }
        bail!("pagination exceeded {PAGE_LIMIT} pages; narrow the query or lower --limit")
    }
}

/// An unsuccessful HTTP status, for callers that treat one status as an answer.
pub struct HttpStatus {
    pub status: u16,
    message: String,
}

impl HttpStatus {
    pub fn is(error: &anyhow::Error, status: u16) -> bool {
        error
            .downcast_ref::<Self>()
            .is_some_and(|error| error.status == status)
    }
}

impl fmt::Display for HttpStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl fmt::Debug for HttpStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

impl Error for HttpStatus {}

struct ResponseDecodeError(serde_json::Error);

impl fmt::Display for ResponseDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "forge returned an invalid JSON response at line {}, column {} ({:?})",
            self.0.line(),
            self.0.column(),
            self.0.classify()
        )
    }
}

impl fmt::Debug for ResponseDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

// Serde errors can contain response values, including credentials echoed by a server.
impl Error for ResponseDecodeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_secure_origins_and_loopback_http() {
        for host in [
            "git.cmu.dev",
            "https://git.cmu.dev",
            "http://127.0.0.1:3000",
            "http://[::1]:3000",
        ] {
            host_url(host).unwrap();
        }
        for host in [
            "http://example.com",
            "https://user:secret@git.cmu.dev",
            "https://git.cmu.dev/api",
            "https://git.cmu.dev?token=secret",
            "file:///tmp/forge",
        ] {
            assert!(host_url(host).is_err());
        }
    }
}
