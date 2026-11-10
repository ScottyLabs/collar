//! Fixed-provider relay; the caller cannot select a user or identity provider.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, ensure};
use jsonwebtoken::jwk::JwkSet;
use serde::Deserialize;
use tiny_http::{Header, Method, Request, Response, Server};

use super::{Endpoints, ForgeToken, client, read_json, secret};

pub fn exchange(
    endpoints: &Endpoints,
    client_secret: &str,
    token: &str,
    keys: &JwkSet,
) -> Result<(u16, Option<ForgeToken>)> {
    super::verify(token, &endpoints.issuer, "collar-broker", keys)?;
    exchange_verified(endpoints, client_secret, token)
}

fn exchange_verified(
    endpoints: &Endpoints,
    client_secret: &str,
    token: &str,
) -> Result<(u16, Option<ForgeToken>)> {
    let body = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs([
            ("client_id", "collar-broker"),
            ("client_secret", client_secret),
            ("token", token),
        ])
        .finish();
    let url = format!(
        "{}/broker/cmu-dev/token",
        endpoints.issuer.as_str().trim_end_matches('/')
    );
    let response = client()?
        .post(url)
        .header("content-type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .map_err(|_| anyhow::anyhow!("identity broker unavailable"))?;
    if !response.status().is_success() {
        #[derive(Deserialize)]
        struct Error {
            error: String,
            #[serde(default)]
            error_description: String,
        }
        let error: Error = read_json(response)?;
        if matches!(
            error.error.as_str(),
            "invalid_grant" | "not_linked" | "token_expired"
        ) || (error.error == "invalid_request"
            && error.error_description == "User not associated to identity provider")
        {
            return Ok((409, None));
        }
        anyhow::bail!("identity provider rejected token retrieval");
    }
    let value: ForgeToken = read_json(response)?;
    secret(&value.access_token)?;
    ensure!(
        value.expires_in > 0,
        "identity provider returned an expired token"
    );
    Ok((200, Some(value)))
}

pub fn run(
    address: std::net::SocketAddr,
    endpoints: Endpoints,
    client_secret: String,
) -> Result<()> {
    ensure!(
        address.ip().is_loopback(),
        "broker must listen on loopback behind the HTTPS proxy"
    );
    secret(&client_secret)?;
    let keys = Arc::new(Mutex::new((Instant::now(), super::keys(&endpoints)?)));
    let server =
        Arc::new(Server::http(address).map_err(|_| anyhow::anyhow!("bind broker listener"))?);
    std::thread::scope(|scope| {
        let mut workers = Vec::new();
        for _ in 0..4 {
            let (server, keys, endpoints, client_secret) = (
                Arc::clone(&server),
                Arc::clone(&keys),
                &endpoints,
                &client_secret,
            );
            workers.push(scope.spawn(move || -> Result<()> {
                loop {
                    let request = server.recv().context("receive broker request")?;
                    let (status, body) = handle(&request, endpoints, client_secret, &keys)
                        .unwrap_or((502, "{\"error\":\"broker_unavailable\"}".into()));
                    let mut response = Response::from_string(body).with_status_code(status);
                    for (name, value) in [
                        ("content-type", "application/json"),
                        ("cache-control", "no-store"),
                        ("pragma", "no-cache"),
                    ] {
                        response.add_header(
                            Header::from_bytes(name, value)
                                .map_err(|_| anyhow::anyhow!("invalid broker response header"))?,
                        );
                    }
                    // A disconnected client does not terminate the broker.
                    let _ = request.respond(response);
                }
            }));
        }
        for worker in workers {
            worker
                .join()
                .map_err(|_| anyhow::anyhow!("broker worker failed"))??;
        }
        Ok(())
    })
}

type Keys = Mutex<(Instant, JwkSet)>;
fn handle(
    request: &Request,
    endpoints: &Endpoints,
    client_secret: &str,
    keys: &Keys,
) -> Result<(u16, String)> {
    if request.method() != &Method::Post
        || request.url() != "/token"
        || request.body_length().is_some_and(|length| length != 0)
    {
        return Ok((400, "{\"error\":\"invalid_request\"}".into()));
    }
    let headers: Vec<_> = request
        .headers()
        .iter()
        .filter(|header| header.field.equiv("authorization"))
        .collect();
    let Some(token) = headers
        .first()
        .filter(|_| headers.len() == 1)
        .and_then(|header| header.value.as_str().strip_prefix("Bearer "))
    else {
        return Ok((401, "{\"error\":\"invalid_token\"}".into()));
    };
    secret(token)?;
    let current = {
        let mut keys = keys
            .lock()
            .map_err(|_| anyhow::anyhow!("signing key cache unavailable"))?;
        if keys.0.elapsed() >= Duration::from_secs(60) {
            *keys = (Instant::now(), super::keys(endpoints)?);
        }
        keys.1.clone()
    };
    if super::verify(token, &endpoints.issuer, "collar-broker", &current).is_err() {
        return Ok((401, "{\"error\":\"invalid_token\"}".into()));
    }
    let (status, value) = exchange_verified(endpoints, client_secret, token)?;
    Ok((
        status,
        match value {
            Some(value) => serde_json::to_string(&value).context("encode broker token")?,
            None => "{\"error\":\"link_required\"}".into(),
        },
    ))
}
