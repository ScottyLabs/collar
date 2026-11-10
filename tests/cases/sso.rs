use super::*;
use collar::sso::{self, Endpoints, Session};
use jsonwebtoken::{Algorithm, EncodingKey, Header};

fn fixture_endpoints(origin: &str) -> Endpoints {
    Endpoints {
        issuer: url::Url::parse(&format!("{origin}/realms/fixture")).unwrap(),
        broker: url::Url::parse(&format!("{origin}/")).unwrap(),
        grafana: url::Url::parse(&format!("{origin}/")).unwrap(),
    }
}
fn identity(issuer: &str, audience: &str, azp: &str, subject: &str, expiry: u64) -> String {
    signed_identity(
        json!({"iss":issuer,"aud":audience,"azp":azp,"sub":subject,"exp":expiry,"iat":sso::now().unwrap(),"nonce":"fixture-nonce"}),
    )
}
fn signed_identity(claims: Value) -> String {
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some("fixture".into());
    jsonwebtoken::encode(
        &header,
        &claims,
        &EncodingKey::from_rsa_der(include_bytes!("../fixtures/sso/key.der")),
    )
    .unwrap()
}
fn session(endpoints: &Endpoints) -> Session {
    Session {
        issuer: endpoints.issuer.to_string(),
        client_id: sso::CLIENT_ID.into(),
        broker: endpoints.broker.to_string(),
        grafana: endpoints.grafana.to_string(),
        subject: "user-a".into(),
        nonce: "fixture-nonce".into(),
        auth_time: Some(100),
        access_token: "keycloak-access".into(),
        refresh_token: "keycloak-refresh".into(),
        expires_at: sso::now().unwrap() + 3600,
    }
}
fn jwks() -> Value {
    serde_json::from_str(include_str!("../fixtures/sso/jwks.json")).unwrap()
}

fn discovery(origin: &str) -> Value {
    let issuer = fixture_endpoints(origin).issuer;
    json!({
        "issuer": issuer,
        "authorization_endpoint": format!("{issuer}/protocol/openid-connect/auth"),
        "token_endpoint": format!("{issuer}/protocol/openid-connect/token"),
        "jwks_uri": format!("{issuer}/protocol/openid-connect/certs"),
        "response_types_supported": ["code"], "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["RS256"],
    })
}

#[test]
fn signatures_issuer_audience_client_expiry_and_subject_are_required() {
    let endpoints = fixture_endpoints("http://127.0.0.1:1");
    let keys = serde_json::from_value(jwks()).unwrap();
    let future = sso::now().unwrap() + 300;
    let valid = identity(
        endpoints.issuer.as_str(),
        "collar-broker",
        "collar",
        "user-a",
        future,
    );
    assert_eq!(
        sso::verify(&valid, &endpoints.issuer, "collar-broker", &keys)
            .unwrap()
            .sub,
        "user-a"
    );
    for (issuer, audience, client, subject, expiry) in [
        (
            "https://other.example/",
            "collar-broker",
            "collar",
            "user-a",
            future,
        ),
        (
            endpoints.issuer.as_str(),
            "grafana",
            "collar",
            "user-a",
            future,
        ),
        (
            endpoints.issuer.as_str(),
            "collar-broker",
            "other",
            "user-a",
            future,
        ),
        (
            endpoints.issuer.as_str(),
            "collar-broker",
            "collar",
            "",
            future,
        ),
        (
            endpoints.issuer.as_str(),
            "collar-broker",
            "collar",
            "user-a",
            1,
        ),
    ] {
        assert!(
            sso::verify(
                &identity(issuer, audience, client, subject, expiry),
                &endpoints.issuer,
                "collar-broker",
                &keys
            )
            .is_err()
        );
    }
    let mut tampered = valid.into_bytes();
    let last = tampered.len() - 10;
    tampered[last] = if tampered[last] == b'A' { b'B' } else { b'A' };
    assert!(
        sso::verify(
            std::str::from_utf8(&tampered).unwrap(),
            &endpoints.issuer,
            "collar-broker",
            &keys
        )
        .is_err()
    );
}

#[test]
fn refresh_verifies_identity_and_rotation_without_retrying_errors() {
    let server = Server::with_responses(|origin| {
        vec![
            Response::json(discovery(origin)),
            Response::json(jwks()),
            Response::json(
                json!({"token_type":"Bearer","access_token":"new-access","refresh_token":"rotated-refresh","expires_in":300,
            "id_token":identity(fixture_endpoints(origin).issuer.as_str(),"collar","collar","user-a",sso::now().unwrap()+300)}),
            ),
        ]
    });
    let endpoints = fixture_endpoints(&server.origin);
    let refreshed = sso::refresh(&endpoints, &session(&endpoints)).unwrap();
    assert_eq!(refreshed.refresh_token, "rotated-refresh");
    let requests = server.finish();
    assert!(requests[2].starts_with("POST /realms/fixture/protocol/openid-connect/token "));
    assert!(requests[2].contains("refresh_token=keycloak-refresh"));
    assert!(!requests[1].contains("keycloak-refresh"));
    for status in [400, 503] {
        let server = Server::with_responses(|origin| {
            vec![
                Response::json(discovery(origin)),
                Response::json(jwks()),
                Response {
                    status,
                    body: "secret-error-body".into(),
                    headers: String::new(),
                },
            ]
        });
        let endpoints = fixture_endpoints(&server.origin);
        let error = sso::refresh(&endpoints, &session(&endpoints))
            .err()
            .unwrap()
            .to_string();
        assert!(!error.contains("secret-error-body"));
        assert!(!error.contains("keycloak-refresh"));
        assert_eq!(server.finish().len(), 3);
    }
}

#[test]
fn refresh_preserves_authentication_and_binds_the_access_token() {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    use sha2::{Digest, Sha256};

    for change in [
        "none",
        "omit-nonce",
        "nonce",
        "auth_time",
        "sub",
        "at_hash",
        "omit-iat",
        "extra-audience",
        "azp",
        "token-type",
    ] {
        let server = Server::with_responses(|origin| {
            let mut claims = json!({
                "iss": fixture_endpoints(origin).issuer.as_str(),
                "aud": "collar", "azp": "collar", "sub": "user-a",
                "exp": sso::now().unwrap() + 300, "iat": sso::now().unwrap(), "nonce": "fixture-nonce",
                "auth_time": 100,
                "at_hash": URL_SAFE_NO_PAD.encode(&Sha256::digest(b"new-access")[..16]),
            });
            match change {
                "omit-nonce" => {
                    claims.as_object_mut().unwrap().remove("nonce");
                }
                "omit-iat" => {
                    claims.as_object_mut().unwrap().remove("iat");
                }
                "extra-audience" => claims["aud"] = json!(["collar", "untrusted-client"]),
                "azp" => claims["azp"] = json!("other-client"),
                "nonce" => claims["nonce"] = json!("other-login"),
                "auth_time" => claims["auth_time"] = json!(101),
                "sub" => claims["sub"] = json!("user-b"),
                "at_hash" => claims["at_hash"] = json!("different-access-token"),
                _ => {}
            }
            vec![
                Response::json(discovery(origin)),
                Response::json(jwks()),
                Response::json(json!({
                    "token_type": if change == "token-type" { "MAC" } else { "Bearer" }, "access_token": "new-access", "refresh_token": "rotated-refresh",
                    "expires_in": 300, "id_token": signed_identity(claims),
                })),
            ]
        });
        let endpoints = fixture_endpoints(&server.origin);
        let result = sso::refresh(&endpoints, &session(&endpoints));
        if matches!(change, "none" | "omit-nonce") {
            let refreshed = result.unwrap();
            assert_eq!(refreshed.nonce, "fixture-nonce");
            assert_eq!(refreshed.auth_time, Some(100));
            assert_eq!(refreshed.refresh_token, "rotated-refresh");
        } else {
            assert!(result.is_err(), "accepted changed {change}");
        }
        assert_eq!(server.finish().len(), 3);
    }
}

#[test]
fn unavailable_signing_keys_do_not_consume_a_refresh_grant() {
    let server = Server::with_responses(|origin| {
        vec![
            Response::json(discovery(origin)),
            Response {
                status: 503,
                body: "private-error".into(),
                headers: String::new(),
            },
        ]
    });
    let endpoints = fixture_endpoints(&server.origin);
    assert!(sso::refresh(&endpoints, &session(&endpoints)).is_err());
    let requests = server.finish();
    assert_eq!(requests.len(), 2);
    assert!(requests.iter().all(|request| request.starts_with("GET ")));
    assert!(
        requests
            .iter()
            .all(|request| !request.contains("keycloak-refresh"))
    );
}

#[test]
fn broker_access_tokens_accept_the_expected_service_audiences() {
    let endpoints = fixture_endpoints("http://127.0.0.1:1");
    let keys = serde_json::from_value(jwks()).unwrap();
    let token = signed_identity(json!({"iss": endpoints.issuer.as_str(),
        "aud": ["collar-broker", "grafana"], "azp": "collar", "sub": "user-a",
        "exp": sso::now().unwrap() + 300}));
    assert!(sso::verify(&token, &endpoints.issuer, "collar-broker", &keys).is_ok());
}

#[test]
fn broker_is_fixed_to_cmu_dev_and_never_returns_refresh_tokens() {
    let server = Server::new(vec![Response::json(
        json!({"access_token":"forge-access","expires_in":300,"refresh_token":"must-not-leave-server"}),
    )]);
    let endpoints = fixture_endpoints(&server.origin);
    let keys = serde_json::from_value(jwks()).unwrap();
    let token = identity(
        endpoints.issuer.as_str(),
        "collar-broker",
        "collar",
        "user-b",
        sso::now().unwrap() + 300,
    );
    let (status, value) = sso::relay::exchange(&endpoints, "server-secret", &token, &keys).unwrap();
    assert_eq!(status, 200);
    let serialized = serde_json::to_string(&value).unwrap();
    assert!(!serialized.contains("must-not-leave-server"));
    let requests = server.finish();
    assert!(requests[0].starts_with("POST /realms/fixture/broker/cmu-dev/token "));
    assert!(requests[0].contains("client_id=collar-broker"));
    assert!(!requests[0].contains("user-b"));
}

#[test]
fn broker_distinguishes_linking_from_upstream_failures() {
    let keys = serde_json::from_value(jwks()).unwrap();
    for error in [
        "not_linked",
        "token_expired",
        "invalid_grant",
        "unauthorized_client",
    ] {
        let server = Server::new(vec![Response {
            status: 400,
            body: json!({"error":error}).to_string(),
            headers: String::new(),
        }]);
        let endpoints = fixture_endpoints(&server.origin);
        let token = identity(
            endpoints.issuer.as_str(),
            "collar-broker",
            "collar",
            "user-a",
            sso::now().unwrap() + 300,
        );
        let result = sso::relay::exchange(&endpoints, "server-secret", &token, &keys);
        if error == "unauthorized_client" {
            assert!(result.is_err());
        } else {
            assert_eq!(result.unwrap().0, 409);
        }
        assert_eq!(server.finish().len(), 1);
    }
}

#[test]
fn session_endpoint_binding_and_query_bounds_fail_before_network() {
    let endpoints = fixture_endpoints("http://127.0.0.1:1");
    let mut session = session(&endpoints);
    session.issuer = "https://other.example/".into();
    assert!(sso::refresh(&endpoints, &session).is_err());
    assert!(sso::forge(&endpoints, &session).is_err());
    assert!(
        sso::logs::read(
            &endpoints,
            &session,
            "project",
            "branch",
            &"a".repeat(40),
            1,
            1
        )
        .is_err()
    );
    for url in [
        "http://idp.example/",
        "https://user:password@idp.example/",
        "https://idp.example/?token=secret",
    ] {
        assert!(sso::secure_url(url).is_err());
    }
}

#[test]
fn deployment_logs_escape_identity_and_keep_tokens_out_of_urls() {
    let timestamp = format!("{}000000000", sso::now().unwrap() - 1);
    let server = Server::new(vec![Response::json(
        json!({"status":"success","data":{"resultType":"streams","result":[{"values":[[timestamp,"fixture log"]]}]}}),
    )]);
    let endpoints = fixture_endpoints(&server.origin);
    let lines = sso::logs::read(
        &endpoints,
        &session(&endpoints),
        "project\"}",
        "branch\\name",
        &"a".repeat(40),
        50,
        60,
    )
    .unwrap();
    assert_eq!(lines[0].line, "fixture log");
    let requests = server.finish();
    assert!(
        requests[0]
            .to_ascii_lowercase()
            .contains("x-jwt-assertion: keycloak-access")
    );
    let first = requests[0].lines().next().unwrap();
    assert!(!first.contains("keycloak-access"));
    let url = url::Url::parse(&format!(
        "{}{}",
        endpoints.grafana.as_str().trim_end_matches('/'),
        first.split_whitespace().nth(1).unwrap()
    ))
    .unwrap();
    let query = url.query_pairs().find(|(key, _)| key == "query").unwrap().1;
    assert!(query.contains(r#"project="project\"}""#));
    assert!(query.contains(r#"branch="branch\\name""#));
}
