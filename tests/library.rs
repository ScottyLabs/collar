use collar::{Api, Config, Method, ReadCache, Repo, authenticated_api, host_url};
use serde_json::{Value, json};
use std::{thread, time::Duration};
use tiny_http::{Header, Response, Server, StatusCode};

#[test]
fn public_client_reuses_authenticated_cache_without_a_cli_subprocess() {
    let server = Server::http("127.0.0.1:0").unwrap();
    let origin = host_url(&format!("http://{}", server.server_addr())).unwrap();
    let worker = thread::spawn(move || {
        for unchanged in [false, true] {
            let request = server
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
                .unwrap();
            assert_eq!(request.url(), "/api/v1/user");
            assert!(request.headers().iter().any(|header| {
                header.field.equiv("Authorization") && header.value.as_str() == "token test-secret"
            }));
            assert_eq!(
                request
                    .headers()
                    .iter()
                    .any(|header| header.field.equiv("If-None-Match")
                        && header.value.as_str() == "\"user-v1\""),
                unchanged
            );
            let response = Response::from_string(if unchanged {
                String::new()
            } else {
                json!({"login":"tester"}).to_string()
            })
            .with_status_code(StatusCode(if unchanged { 304 } else { 200 }))
            .with_header(Header::from_bytes("ETag", "\"user-v1\"").unwrap());
            request.respond(response).unwrap();
        }
    });
    let cache = tempfile::tempdir().unwrap();
    let api = Api::new(origin, Some("test-secret".into()))
        .unwrap()
        .with_read_cache(ReadCache {
            directory: Some(cache.path().into()),
            refresh: false,
        });
    for _ in 0..2 {
        let user: Value = api.json(Method::GET, &["user"], &[], None).unwrap();
        assert_eq!(user["login"], "tester");
    }
    worker.join().unwrap();
}

#[test]
fn public_authenticated_client_can_read_typed_prs_with_an_isolated_config() {
    let server = Server::http("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", server.server_addr());
    let worker = thread::spawn(move || {
        let request = server
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap();
        assert_eq!(request.url(), "/api/v1/repos/ScottyLabs/example/pulls/7");
        assert!(
            !request
                .headers()
                .iter()
                .any(|header| header.field.equiv("Authorization"))
        );
        request.respond(Response::from_string(json!({
            "number":7,"title":"Example","body":"Description","state":"open","merged":false,"draft":false,
            "html_url":"https://example.invalid/pulls/7","user":{"login":"tester"},
            "head":{"ref":"feature","sha":"head"},"base":{"ref":"main","sha":"base"}
        }).to_string())).unwrap();
    });
    let cache = tempfile::tempdir().unwrap();
    let mut config = Config::load(None, true, false).unwrap();
    config.read_cache.directory = Some(cache.path().into());
    let repo = Repo::parse("ScottyLabs/example", Some(&origin)).unwrap();
    let api = authenticated_api(repo.origin.clone(), &config).unwrap();
    let pull: collar::pr::Pull = collar::pr::get(&api, &repo, 7).unwrap();
    let head: &collar::pr::Branch = &pull.head;
    assert_eq!(head.sha, "head");
    assert_eq!(pull.title, "Example");
    worker.join().unwrap();
}
