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

#[test]
fn get_cache_is_scoped_to_credentials_query_and_host() {
    let temp = tempfile::tempdir().unwrap();
    let server = Server::new(
        (0..3)
            .map(|value| Response {
                status: 200,
                body: json!({"value":value}).to_string(),
                headers: format!("ETag: \"partition-{value}\"\r\n"),
            })
            .collect(),
    );
    for (token, query, value) in [
        ("test-secret", "one", 0),
        ("different-secret", "one", 1),
        ("test-secret", "two", 2),
    ] {
        let output = server
            .command(true)
            .env("COLLAR_TOKEN", token)
            .arg("--cache-dir")
            .arg(temp.path())
            .args([
                "api",
                "user",
                "--method",
                "GET",
                "-f",
                &format!("q={query}"),
            ])
            .output()
            .unwrap();
        success(&output);
        assert_eq!(
            serde_json::from_slice::<Value>(&output.stdout).unwrap()["value"],
            value
        );
    }
    let requests = server.finish();
    assert_eq!(requests.len(), 3);
    assert!(
        requests
            .iter()
            .all(|request| !request.to_lowercase().contains("if-none-match:"))
    );
    let other = Server::new(vec![Response::json(json!({"value":"other host"}))]);
    let output = other
        .command(true)
        .arg("--cache-dir")
        .arg(temp.path())
        .args(["api", "user", "--method", "GET", "-f", "q=one"])
        .output()
        .unwrap();
    success(&output);
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap()["value"],
        "other host"
    );
    assert_eq!(other.finish().len(), 1);
}

#[test]
fn corrupt_or_unavailable_get_cache_does_not_break_successful_reads() {
    let temp = tempfile::tempdir().unwrap();
    let server = Server::new(
        (0..3)
            .map(|value| Response::json(json!({"value":value})))
            .collect(),
    );
    let run = |root: &std::path::Path| {
        server
            .command(false)
            .arg("--cache-dir")
            .arg(root)
            .args(["api", "user"])
            .output()
            .unwrap()
    };
    success(&run(temp.path()));
    let entry = cached_entry(temp.path());
    std::fs::write(entry, b"invalid").unwrap();
    let output = run(temp.path());
    success(&output);
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap()["value"],
        1
    );
    let unavailable = temp.path().join("not-a-directory");
    std::fs::write(&unavailable, b"file").unwrap();
    success(&run(&unavailable));
    assert_eq!(server.finish().len(), 3);
}

fn cached_entry(root: &std::path::Path) -> std::path::PathBuf {
    let partition = std::fs::read_dir(root.join("api/v2"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    std::fs::read_dir(partition)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path()
}

#[test]
fn default_cache_revalidates_etags_and_updates_changed_responses() {
    let temp = tempfile::tempdir().unwrap();
    let server = Server::new(vec![
        Response {
            status: 200,
            body: json!({"value":"one"}).to_string(),
            headers: "ETag: \"one\"\r\n".into(),
        },
        Response {
            status: 304,
            body: String::new(),
            headers: "ETag: W/\"one\"\r\n".into(),
        },
        Response {
            status: 200,
            body: json!({"value":"two"}).to_string(),
            headers: "ETag: \"two\"\r\n".into(),
        },
    ]);
    let run = || {
        server
            .command(true)
            .arg("--cache-dir")
            .arg(temp.path())
            .args(["api", "user"])
            .output()
            .unwrap()
    };
    let first = run();
    success(&first);
    let unchanged = run();
    success(&unchanged);
    assert_eq!(first.stdout, unchanged.stdout);
    let changed = run();
    success(&changed);
    assert_eq!(
        serde_json::from_slice::<Value>(&changed.stdout).unwrap()["value"],
        "two"
    );
    let requests = server.finish();
    assert_eq!(requests.len(), 3);
    assert!(
        requests[1]
            .to_lowercase()
            .contains("if-none-match: \"one\"")
    );
    assert!(
        requests[2]
            .to_lowercase()
            .contains("if-none-match: w/\"one\"")
    );
}

#[test]
fn default_cache_uses_last_modified_when_no_etag_is_available() {
    let server = Server::new(vec![
        Response {
            status: 200,
            body: json!({"value":1}).to_string(),
            headers: "Last-Modified: Tue, 17 Nov 2026 10:00:00 GMT\r\n".into(),
        },
        Response {
            status: 304,
            body: String::new(),
            headers: String::new(),
        },
    ]);
    for _ in 0..2 {
        success(
            &server
                .command(false)
                .args(["api", "user"])
                .output()
                .unwrap(),
        );
    }
    let requests = server.finish();
    assert!(
        requests[1]
            .to_lowercase()
            .contains("if-modified-since: tue, 17 nov 2026 10:00:00 gmt")
    );
}

#[test]
fn refresh_bypasses_validators_and_updates_the_cache() {
    let server = Server::new(vec![
        Response {
            status: 200,
            body: json!({"value":"old"}).to_string(),
            headers: "ETag: \"old\"\r\n".into(),
        },
        Response {
            status: 200,
            body: json!({"value":"new"}).to_string(),
            headers: "ETag: \"new\"\r\n".into(),
        },
        Response {
            status: 304,
            body: String::new(),
            headers: String::new(),
        },
    ]);
    for refresh in [false, true, false] {
        let mut command = server.command(true);
        if refresh {
            command.arg("--refresh");
        }
        success(&command.args(["api", "user"]).output().unwrap());
    }
    let requests = server.finish();
    assert_eq!(requests.len(), 3);
    assert!(!requests[1].to_lowercase().contains("if-none-match:"));
    assert!(
        requests[2]
            .to_lowercase()
            .contains("if-none-match: \"new\"")
    );
}

#[test]
fn writes_invalidate_prior_read_responses() {
    let temp = tempfile::tempdir().unwrap();
    let server = Server::new(vec![
        Response {
            status: 200,
            body: json!({"value":"old"}).to_string(),
            headers: "ETag: \"old\"\r\n".into(),
        },
        Response::json(json!({"value":"updated"})),
        Response::json(json!({"value":"new"})),
    ]);
    let read = || {
        server
            .command(true)
            .arg("--cache-dir")
            .arg(temp.path())
            .args(["api", "user"])
            .output()
            .unwrap()
    };
    success(&read());
    success(
        &server
            .command(true)
            .arg("--cache-dir")
            .arg(temp.path())
            .args(["api", "user", "--method", "PATCH", "-f", "value=updated"])
            .output()
            .unwrap(),
    );
    let new = read();
    success(&new);
    assert_eq!(
        serde_json::from_slice::<Value>(&new.stdout).unwrap()["value"],
        "new"
    );
    let requests = server.finish();
    assert_eq!(requests.len(), 3);
    assert!(!requests[2].to_lowercase().contains("if-none-match:"));
}

#[test]
fn default_revalidation_never_hides_permission_errors() {
    let server = Server::new(vec![
        Response {
            status: 200,
            body: json!({"value":"old"}).to_string(),
            headers: "ETag: \"old\"\r\n".into(),
        },
        Response {
            status: 403,
            body: "test-secret".into(),
            headers: String::new(),
        },
    ]);
    success(&server.command(true).args(["api", "user"]).output().unwrap());
    failure(
        &server.command(true).args(["api", "user"]).output().unwrap(),
        "HTTP 403",
    );
    assert_eq!(server.finish().len(), 2);
}

#[test]
fn no_store_and_no_cache_prevent_unvalidated_reuse() {
    for directive in ["no-store", "no-cache"] {
        let temp = tempfile::tempdir().unwrap();
        let server = Server::new(vec![
            Response {
                status: 200,
                body: json!({"value":"old"}).to_string(),
                headers: format!("Cache-Control: {directive}\r\n"),
            },
            Response::json(json!({"value":"new"})),
        ]);
        let run = || {
            server
                .command(false)
                .arg("--cache-dir")
                .arg(temp.path())
                .args(["api", "user"])
                .output()
                .unwrap()
        };
        success(&run());
        if directive == "no-store" {
            assert!(!temp.path().join("api/v2").exists());
        }
        let new = run();
        success(&new);
        assert_eq!(
            serde_json::from_slice::<Value>(&new.stdout).unwrap()["value"],
            "new"
        );
        assert_eq!(server.finish().len(), 2);
    }
}

#[test]
fn successful_cache_entries_are_private() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let server = Server::new(vec![Response::json(json!({"value":1}))]);
    let output = server
        .command(true)
        .arg("--cache-dir")
        .arg(temp.path())
        .args(["api", "user"])
        .output()
        .unwrap();
    success(&output);
    assert_eq!(server.finish().len(), 1);
    let entry = cached_entry(temp.path());
    assert_eq!(
        std::fs::metadata(&entry).unwrap().permissions().mode() & 0o077,
        0
    );
    assert!(
        !entry
            .file_name()
            .unwrap()
            .to_string_lossy()
            .contains("test-secret")
    );
    assert!(
        !std::fs::read_to_string(entry)
            .unwrap()
            .contains("test-secret")
    );
}
