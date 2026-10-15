use super::*;

#[test]
fn pagination_limit_fails_instead_of_returning_partial_results() {
    let server = Server::new(
        (0..100)
            .map(|_| Response::json(json!([pull("head")])))
            .collect(),
    );
    let output = server
        .command(false)
        .args(["pr", "list", "--limit", "101", "--json"])
        .output()
        .unwrap();
    failure(&output, "pagination exceeded");
    assert!(output.stdout.is_empty());
    assert_eq!(server.finish().len(), 100);
}

#[test]
fn errors_do_not_expose_response_bodies_or_follow_redirects() {
    for (status, message) in [
        (403, "HTTP 403"),
        (302, "redirect refused"),
        (422, "server rejected"),
    ] {
        let server = Server::new(vec![Response {
            status,
            body: "echoed credential: test-secret".into(),
            headers: "Location: http://127.0.0.1:1/steal\r\n".into(),
        }]);
        let output = server.command(true).args(["api", "user"]).output().unwrap();
        failure(&output, message);
        assert_eq!(server.finish().len(), 1);
    }
}

#[test]
fn malformed_json_and_missing_fields_fail() {
    for body in ["not json", "{}"] {
        let server = Server::new(vec![Response {
            status: 200,
            body: body.into(),
            headers: String::new(),
        }]);
        let output = server
            .command(false)
            .args(["pr", "view", "42"])
            .output()
            .unwrap();
        failure(&output, "invalid JSON response");
        server.finish();
    }
}

#[test]
fn typed_response_errors_do_not_echo_secret_values() {
    let mut invalid = pull("head");
    invalid["number"] = json!("test-secret");
    let server = Server::new(vec![Response::json(invalid)]);
    let output = server
        .command(true)
        .args(["pr", "view", "42"])
        .output()
        .unwrap();
    failure(&output, "invalid JSON response");
    server.finish();
}

#[test]
fn empty_lists_reject_unknown_json_fields_before_requesting() {
    let server = Server::new(vec![]);
    let output = server
        .command(false)
        .args(["pr", "list", "--json", "titel"])
        .output()
        .unwrap();
    failure(&output, "unsupported JSON field");
    assert!(output.stdout.is_empty());
    server.finish();
}

#[test]
fn raw_api_matches_gh_field_types_and_post_inference() {
    let server = Server::new(vec![Response::json(json!({ "id": 1 }))]);
    let output = server
        .command(true)
        .args([
            "api",
            "repos/ScottyLabs/terrier/issues",
            "-f",
            "title=123",
            "-F",
            "closed=false",
        ])
        .output()
        .unwrap();
    success(&output);
    let requests = server.finish();
    assert!(requests[0].starts_with("POST "));
    let body: Value = serde_json::from_str(requests[0].split_once("\r\n\r\n").unwrap().1).unwrap();
    assert_eq!(body, json!({ "title": "123", "closed": false }));
}

#[test]
fn rejects_unsafe_targets_and_reviews_without_credentials() {
    for args in [
        vec!["--host", "http://example.com", "api", "user"],
        vec!["api", "https://evil.example/user"],
        vec!["api", "../user"],
        vec![
            "-R",
            "ScottyLabs/terrier",
            "pr",
            "review",
            "42",
            "--approve",
        ],
        vec!["pr", "review", "42", "--approve", "--comment"],
    ] {
        let output = command().args(args).output().unwrap();
        assert!(!output.status.success());
    }
}

#[test]
fn does_not_send_default_host_token_to_another_host() {
    let server = Server::new(vec![Response::json(json!({ "version": "16" }))]);
    let output = command()
        .env("COLLAR_TOKEN", "test-secret")
        .args(["--host", &server.origin, "api", "version"])
        .output()
        .unwrap();
    success(&output);
    assert!(!server.finish()[0].to_lowercase().contains("authorization:"));
}
