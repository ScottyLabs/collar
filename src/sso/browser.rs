use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, ensure};
use tiny_http::{Response, Server};
use url::Url;

use super::{Endpoints, Session, oidc::Oidc};

pub fn login(endpoints: &Endpoints, link: bool) -> Result<Session> {
    login_with(endpoints, link, |url| {
        let program = if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };
        let mut child = Command::new(program)
            .arg(url.as_str())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .context("open SSO browser")?;
        let started = Instant::now();
        loop {
            if let Some(status) = child.try_wait().context("check browser launcher")? {
                ensure!(status.success(), "could not open SSO browser");
                break;
            }
            if started.elapsed() >= Duration::from_secs(10) {
                let _ = child.kill();
                let _ = child.wait();
                anyhow::bail!("browser launcher timed out");
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        Ok(())
    })
}

fn login_with(
    endpoints: &Endpoints,
    link: bool,
    open: impl FnOnce(&Url) -> Result<()>,
) -> Result<Session> {
    let server =
        Server::http("127.0.0.1:0").map_err(|_| anyhow::anyhow!("bind SSO loopback callback"))?;
    let address = server
        .server_addr()
        .to_ip()
        .context("SSO callback has no IP address")?;
    let redirect = Url::parse(&format!("http://{address}/callback"))?;
    let oidc = Oidc::discover(endpoints)?.with_redirect(&redirect)?;
    let (authorization, state, nonce, verifier) = oidc.authorization(link);
    open(&authorization)?;
    let deadline = Instant::now() + Duration::from_secs(300);
    loop {
        ensure!(
            Instant::now() < deadline,
            "SSO browser login timed out after five minutes"
        );
        let Some(request) = server
            .recv_timeout(Duration::from_millis(250))
            .context("read SSO callback")?
        else {
            continue;
        };
        let result = callback(request.url(), &redirect, state.secret(), &endpoints.issuer);
        let status = if result.is_ok() { 200 } else { 400 };
        request
            .respond(
                Response::from_string(if result.is_ok() {
                    "Sign-in received. You can close this tab."
                } else {
                    "Invalid sign-in callback."
                })
                .with_status_code(status),
            )
            .context("reply to SSO callback")?;
        let code = match result {
            Ok(code) => code,
            Err(error) if error.is::<LoginDenied>() => return Err(error),
            Err(_) => continue,
        };
        return oidc.redeem(endpoints, code, verifier, &nonce);
    }
}

fn callback(target: &str, redirect: &Url, state: &str, issuer: &Url) -> Result<String> {
    ensure!(
        target.starts_with("/callback?") && !target.starts_with("//"),
        "invalid SSO callback path"
    );
    let url = redirect.join(target).context("invalid SSO callback")?;
    let query: Vec<_> = url.query_pairs().collect();
    let values = |name: &str| {
        query
            .iter()
            .filter(|(key, _)| key == name)
            .map(|(_, value)| value.as_ref())
            .collect::<Vec<_>>()
    };
    ensure!(values("state") == [state], "SSO callback state mismatch");
    let issuers = values("iss");
    ensure!(
        issuers.is_empty() || issuers == [issuer.as_str()],
        "SSO callback issuer mismatch"
    );
    if !values("error").is_empty() {
        anyhow::bail!(LoginDenied);
    }
    let codes = values("code");
    ensure!(
        codes.len() == 1 && !codes[0].is_empty(),
        "SSO callback has no unique code"
    );
    Ok(codes[0].to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn callback_requires_unique_state_and_code() {
        let redirect = Url::parse("http://127.0.0.1:1234/callback").unwrap();
        let issuer = Url::parse("https://idp.example/realms/test").unwrap();
        assert_eq!(
            callback("/callback?state=bound&code=ok", &redirect, "bound", &issuer).unwrap(),
            "ok"
        );
        for target in [
            "/callback?state=other&code=ok",
            "/callback?state=bound&state=bound&code=ok",
            "/callback?state=bound&code=a&code=b",
            "/callback?state=bound&error=denied",
            "/callback?state=bound&code=ok&iss=https://other.example/",
            "/callback?state=bound&code=ok&iss=https://idp.example/realms/test&iss=https://idp.example/realms/test",
            "//other/callback?state=bound&code=ok",
        ] {
            assert!(callback(target, &redirect, "bound", &issuer).is_err());
        }
    }
}

#[derive(Debug)]
struct LoginDenied;
impl std::fmt::Display for LoginDenied {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("SSO browser login denied")
    }
}
impl std::error::Error for LoginDenied {}

#[cfg(test)]
mod flow_tests {
    use super::*;
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    use sha2::{Digest, Sha256};
    use std::collections::BTreeMap;
    use std::sync::mpsc;

    #[test]
    fn browser_flow_binds_pkce_nonce_redirect_and_state() {
        for correct_nonce in [true, false] {
            let idp = Server::http("127.0.0.1:0").unwrap();
            let base = format!("http://{}", idp.server_addr());
            let endpoints = Endpoints {
                issuer: Url::parse(&format!("{base}/realms/test")).unwrap(),
                broker: Url::parse(&format!("{base}/")).unwrap(),
                grafana: Url::parse(&format!("{base}/")).unwrap(),
            };
            let issuer = endpoints.issuer.to_string();
            let (sender, receiver) = mpsc::channel::<BTreeMap<String, String>>();
            let task = std::thread::spawn(move || {
                let request = idp.recv_timeout(Duration::from_secs(5)).unwrap().unwrap();
                assert_eq!(
                    request.url(),
                    "/realms/test/.well-known/openid-configuration"
                );
                request.respond(Response::from_string(serde_json::json!({
                    "issuer": issuer, "authorization_endpoint": format!("{issuer}/protocol/openid-connect/auth"),
                    "token_endpoint": format!("{issuer}/protocol/openid-connect/token"),
                    "jwks_uri": format!("{issuer}/protocol/openid-connect/certs"),
                    "response_types_supported": ["code"], "subject_types_supported": ["public"],
                    "id_token_signing_alg_values_supported": ["RS256"],
                }).to_string()).with_header(tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap())).unwrap();
                let request = idp.recv_timeout(Duration::from_secs(5)).unwrap().unwrap();
                assert_eq!(request.url(), "/realms/test/protocol/openid-connect/certs");
                request
                    .respond(
                        Response::from_string(include_str!("../../tests/fixtures/sso/jwks.json"))
                            .with_header(
                                tiny_http::Header::from_bytes("Content-Type", "application/json")
                                    .unwrap(),
                            ),
                    )
                    .unwrap();
                let authorization = receiver.recv_timeout(Duration::from_secs(5)).unwrap();
                let mut request = idp.recv_timeout(Duration::from_secs(5)).unwrap().unwrap();
                assert_eq!(request.url(), "/realms/test/protocol/openid-connect/token");
                let mut body = String::new();
                request.as_reader().read_to_string(&mut body).unwrap();
                let form: BTreeMap<_, _> = url::form_urlencoded::parse(body.as_bytes())
                    .into_owned()
                    .collect();
                assert_eq!(form["redirect_uri"], authorization["redirect_uri"]);
                assert_eq!(form["code"], "fixture-code");
                assert_eq!(
                    URL_SAFE_NO_PAD.encode(Sha256::digest(form["code_verifier"].as_bytes())),
                    authorization["code_challenge"]
                );
                let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
                header.kid = Some("fixture".into());
                let nonce = if correct_nonce {
                    authorization["nonce"].as_str()
                } else {
                    "wrong-nonce"
                };
                let id_token = jsonwebtoken::encode(&header, &serde_json::json!({"iss":issuer,"aud":"collar","azp":"collar","sub":"fixture-user","exp":super::super::now().unwrap()+300,"iat":super::super::now().unwrap(),"nonce":nonce}),
                    &jsonwebtoken::EncodingKey::from_rsa_der(include_bytes!("../../tests/fixtures/sso/key.der"))).unwrap();
                request.respond(Response::from_string(serde_json::json!({"token_type":"Bearer","access_token":"fixture-access","refresh_token":"fixture-refresh","id_token":id_token,"expires_in":300}).to_string()).with_header(tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap())).unwrap();
            });
            let result = login_with(&endpoints, false, |authorization| {
                let query: BTreeMap<_, _> = authorization.query_pairs().into_owned().collect();
                assert_eq!(query["scope"], "openid profile email");
                assert_eq!(query["code_challenge_method"], "S256");
                assert!(!query.contains_key("kc_action"));
                let mut callback = Url::parse(&query["redirect_uri"]).unwrap();
                callback
                    .query_pairs_mut()
                    .extend_pairs([("state", query["state"].as_str()), ("code", "fixture-code")]);
                sender.send(query).unwrap();
                std::thread::spawn(move || {
                    let client = super::super::client().unwrap();
                    assert_eq!(
                        client
                            .get(callback.join("/favicon.ico").unwrap())
                            .send()
                            .unwrap()
                            .status()
                            .as_u16(),
                        400
                    );
                    assert_eq!(client.get(callback).send().unwrap().status().as_u16(), 200);
                });
                Ok(())
            });
            if correct_nonce {
                assert_eq!(result.unwrap().subject, "fixture-user");
            } else {
                assert!(
                    result
                        .err()
                        .unwrap()
                        .to_string()
                        .contains("identity token verification failed")
                );
            }
            task.join().unwrap();
        }
    }
}
