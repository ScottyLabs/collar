use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::{Command, Output};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tempfile::TempDir;

fn isolated_command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_collar"));
    command
        .env_remove("COLLAR_CONFIG")
        .env_remove("COLLAR_HOST")
        .env_remove("COLLAR_TOKEN")
        .env_remove("COLLAR_GOVERNANCE")
        .env_remove("COLLAR_INFRASTRUCTURE");
    command
}

fn command() -> Command {
    let mut command = isolated_command();
    command.arg("--no-config");
    command
}

struct Response {
    status: u16,
    body: String,
    headers: String,
}

impl Response {
    fn json(body: Value) -> Self {
        Self {
            status: 200,
            body: body.to_string(),
            headers: String::new(),
        }
    }
}

struct Server {
    origin: String,
    task: thread::JoinHandle<Vec<String>>,
}

impl Server {
    fn new(responses: Vec<Response>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let task = thread::spawn(move || {
            let mut requests = Vec::new();
            for response in responses {
                let deadline = Instant::now() + Duration::from_secs(10);
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(
                                Instant::now() < deadline,
                                "expected request was not received"
                            );
                            thread::sleep(Duration::from_millis(5));
                        }
                        Err(error) => panic!("accept: {error}"),
                    }
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut buffer = [0; 4096];
                loop {
                    let count = stream.read(&mut buffer).unwrap();
                    assert!(count > 0, "request ended before its body");
                    bytes.extend_from_slice(&buffer[..count]);
                    if let Some(index) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                        let header = std::str::from_utf8(&bytes[..index]).unwrap();
                        let length: usize = header
                            .lines()
                            .find_map(|line| {
                                line.to_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|value| value.trim().parse().unwrap())
                            })
                            .unwrap_or(0);
                        if bytes.len() >= index + 4 + length {
                            break;
                        }
                    }
                }
                requests.push(String::from_utf8(bytes).unwrap());
                write!(
                    stream,
                    "HTTP/1.1 {} Test\r\nContent-Length: {}\r\nConnection: close\r\n{}\r\n{}",
                    response.status,
                    response.body.len(),
                    response.headers,
                    response.body
                )
                .unwrap();
            }
            requests
        });
        Self { origin, task }
    }

    fn command(&self, token: bool) -> Command {
        let mut command = command();
        command
            .env("COLLAR_HOST", &self.origin)
            .args(["-R", "ScottyLabs/terrier"]);
        if token {
            command.env("COLLAR_TOKEN", "test-secret");
        }
        command
    }

    fn finish(self) -> Vec<String> {
        self.task.join().unwrap()
    }
}

fn success(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn failure(output: &Output, message: &str) {
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(message),
        "expected {message:?}, got {stderr}"
    );
    assert!(!stderr.contains("test-secret"));
}

fn pull(sha: &str) -> Value {
    json!({ "number": 42, "title": "Review me", "body": null, "state": "open", "merged": false, "draft": false, "html_url": "https://git.cmu.dev/ScottyLabs/terrier/pulls/42", "user": { "login": "contributor" }, "head": { "ref": "feat/collar", "sha": sha }, "base": { "ref": "main", "sha": "base" } })
}

#[test]
fn list_follows_short_pages_and_projects_gh_fields() {
    let server = Server::new(vec![
        Response::json(json!([pull("head")])),
        Response::json(json!([pull("head2")])),
        Response::json(json!([])),
    ]);
    let output = server
        .command(false)
        .args(["pr", "list", "--json", "number,headRefOid"])
        .output()
        .unwrap();
    success(&output);
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        json!([{ "number": 42, "headRefOid": "head" }, { "number": 42, "headRefOid": "head2" }])
    );
    let requests = server.finish();
    assert!(
        requests[0]
            .starts_with("GET /api/v1/repos/ScottyLabs/terrier/pulls?state=open&page=1&limit=50 ")
    );
    assert!(requests[1].contains("page=2"));
    assert!(!requests[0].to_lowercase().contains("authorization:"));
}

#[test]
fn submits_one_commit_pinned_review_with_inline_comments() {
    let comments = TempDir::new().unwrap();
    let path = comments.path().join("comments.json");
    std::fs::write(
        &path,
        json!([{ "path": "src/main.rs", "line": 1, "side": "new", "body": "Explain this" }])
            .to_string(),
    )
    .unwrap();
    let server = Server::new(vec![
        Response::json(pull("reviewed-head")),
        Response { status: 200, body: "diff --git a/src/main.rs b/src/main.rs\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1 +1 @@\n-old\n+new\n".into(), headers: String::new() },
        Response::json(pull("reviewed-head")),
        Response::json(json!({ "id": 9 })),
    ]);
    let output = server
        .command(true)
        .args([
            "pr",
            "review",
            "42",
            "--request-changes",
            "--body",
            "Please explain",
            "--comments-file",
        ])
        .arg(&path)
        .output()
        .unwrap();
    success(&output);
    let requests = server.finish();
    assert_eq!(requests.len(), 4);
    assert!(requests[3].starts_with("POST /api/v1/repos/ScottyLabs/terrier/pulls/42/reviews "));
    assert!(
        requests[3]
            .to_lowercase()
            .contains("authorization: token test-secret")
    );
    let body: Value = serde_json::from_str(requests[3].split_once("\r\n\r\n").unwrap().1).unwrap();
    assert_eq!(
        body,
        json!({ "event": "REQUEST_CHANGES", "body": "Please explain", "commit_id": "reviewed-head", "comments": [{ "path": "src/main.rs", "body": "Explain this", "old_position": 0, "new_position": 1 }] })
    );
}

#[test]
fn refuses_changed_heads_without_posting() {
    let server = Server::new(vec![
        Response::json(pull("before")),
        Response::json(pull("after")),
    ]);
    let output = server
        .command(true)
        .args(["pr", "review", "42", "--approve"])
        .output()
        .unwrap();
    failure(&output, "head changed");
    assert!(
        server
            .finish()
            .iter()
            .all(|request| request.starts_with("GET "))
    );
}

#[test]
fn refuses_invalid_inline_anchors_without_posting() {
    let comments = TempDir::new().unwrap();
    let path = comments.path().join("comments.json");
    std::fs::write(
        &path,
        json!([{ "path": "src/main.rs", "line": 99, "side": "new", "body": "Outside the diff" }])
            .to_string(),
    )
    .unwrap();
    let server = Server::new(vec![
        Response::json(pull("head")),
        Response { status: 200, body: "diff --git a/src/main.rs b/src/main.rs\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1 +1 @@\n-old\n+new\n".into(), headers: String::new() },
    ]);
    let output = server
        .command(true)
        .args(["pr", "review", "42", "--comment", "--comments-file"])
        .arg(&path)
        .output()
        .unwrap();
    failure(&output, "not a commentable line");
    assert!(
        server
            .finish()
            .iter()
            .all(|request| request.starts_with("GET "))
    );
}

#[test]
fn pr_urls_must_belong_to_the_selected_repository() {
    let server = Server::new(vec![]);
    let selector = format!("{}/ScottyLabs/other/pulls/42", server.origin);
    let output = server
        .command(false)
        .args(["pr", "view", &selector])
        .output()
        .unwrap();
    failure(&output, "selected repository");
    server.finish();
}

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
fn a_pr_closed_during_review_is_not_submitted() {
    let mut closed = pull("head");
    closed["state"] = json!("closed");
    let server = Server::new(vec![Response::json(pull("head")), Response::json(closed)]);
    let output = server
        .command(true)
        .args(["pr", "review", "42", "--approve"])
        .output()
        .unwrap();
    failure(&output, "PR closed while preparing");
    assert!(
        server
            .finish()
            .iter()
            .all(|request| request.starts_with("GET "))
    );
}

#[test]
fn broken_upstream_remote_does_not_silently_use_origin() {
    let directory = TempDir::new().unwrap();
    for args in [
        vec!["init", "--quiet"],
        vec![
            "config",
            "remote.upstream.fetch",
            "+refs/heads/*:refs/remotes/upstream/*",
        ],
        vec![
            "remote",
            "add",
            "origin",
            "https://git.cmu.dev/ScottyLabs/terrier.git",
        ],
    ] {
        assert!(
            Command::new("git")
                .args(args)
                .current_dir(directory.path())
                .status()
                .unwrap()
                .success()
        );
    }
    let output = command()
        .current_dir(directory.path())
        .args(["repo", "view"])
        .output()
        .unwrap();
    failure(&output, "configured remote upstream");
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
fn governance_uses_nested_project_namespace_and_inherited_leads() {
    let checkout = TempDir::new().unwrap();
    std::fs::create_dir_all(checkout.path().join("data/teams")).unwrap();
    let file = checkout.path().join("data/teams/team.toml");
    std::fs::write(&file, "[team]\nslug = 'tech'\nname = 'Tech'\nleads = ['Lead']\n[[team.projects]]\nslug = 'collie'\nname = 'Collie'\nleads = ['lead', 'Other']\n[[team.projects.repos]]\nname = 'collie-api'\n[team.projects.repos.features]\nkennel = {}\n").unwrap();
    let output = command()
        .args(["governance", "project", "view", "collie-api", "--checkout"])
        .arg(checkout.path())
        .args(["--json", "slug,secretNamespace,leads,repos"])
        .output()
        .unwrap();
    success(&output);
    let project: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(project["secretNamespace"], "collie");
    assert_eq!(project["leads"], json!(["Lead", "Other"]));
    assert_eq!(project["repos"][0]["features"]["kennel"], json!({}));
    std::fs::write(file, "[team]\nname = 'Missing slug'\n").unwrap();
    let output = command()
        .args(["governance", "project", "list", "--checkout"])
        .arg(checkout.path())
        .output()
        .unwrap();
    failure(&output, "missing field");
}

#[test]
fn infrastructure_discovers_configurations_and_refuses_missing_services() {
    let checkout = TempDir::new().unwrap();
    let host = checkout.path().join("modules/hosts/deploy-01");
    std::fs::create_dir_all(&host).unwrap();
    std::fs::create_dir_all(checkout.path().join("modules/roles")).unwrap();
    std::fs::write(host.join("configuration.nix"), "{}").unwrap();
    std::fs::write(host.join("kennel.nix"), "{}").unwrap();
    std::fs::write(checkout.path().join("modules/roles/deploy-01.nix"), "{}").unwrap();
    let output = command()
        .args(["infra", "service", "kennel", "--checkout"])
        .arg(checkout.path())
        .args(["--json", "host"])
        .output()
        .unwrap();
    success(&output);
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        json!([{ "host": "deploy-01" }])
    );
    let output = command()
        .args(["infra", "service", "missing", "--checkout"])
        .arg(checkout.path())
        .output()
        .unwrap();
    failure(&output, "no host configuration");
}

#[test]
fn checks_use_the_latest_status_and_fail_for_pending_checks() {
    let server = Server::new(vec![
        Response::json(pull("head")),
        Response::json(json!([
            { "id": 1, "context": "ci", "status": "success" },
            { "id": 2, "context": "ci", "status": "pending" }
        ])),
        Response::json(json!([])),
    ]);
    let output = server
        .command(false)
        .args(["pr", "checks", "42", "--json", "name,state"])
        .output()
        .unwrap();
    failure(&output, "not successful");
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        json!([{ "name": "ci", "state": "pending" }])
    );
    server.finish();
}

#[test]
fn kennel_reports_marked_comments_without_claiming_live_health() {
    let server = Server::new(vec![
        Response::json(pull("head")),
        Response::json(json!([{ "id": 1, "context": "kennel/deploy", "status": "success" }])),
        Response::json(json!([])),
        Response::json(json!([
            { "body": "unrelated", "html_url": "https://example.com/1" },
            { "body": "<!-- kennel-deployment -->\nPreview: https://collie-pr-42.scottylabs.net", "html_url": "https://example.com/2" }
        ])),
        Response::json(json!([])),
    ]);
    let output = server
        .command(false)
        .args(["kennel", "status", "42", "--json"])
        .output()
        .unwrap();
    success(&output);
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["liveHealthVerified"], false);
    assert_eq!(report["comments"].as_array().unwrap().len(), 1);
    server.finish();
}

#[test]
fn saves_checkout_defaults_and_resolves_relative_paths() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("settings/config.toml");
    let checkout = temp.path().join("infra");
    std::fs::create_dir_all(checkout.join("modules/hosts/test")).unwrap();
    std::fs::create_dir_all(checkout.join("modules/roles")).unwrap();
    std::fs::write(checkout.join("modules/hosts/test/configuration.nix"), "{}").unwrap();
    std::fs::write(checkout.join("modules/roles/test.nix"), "{}").unwrap();
    std::fs::write(checkout.join("flake.nix"), "{}").unwrap();
    let output = isolated_command()
        .arg("--config")
        .arg(&path)
        .args(["config", "set", "infrastructure"])
        .arg(&checkout)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = isolated_command()
        .arg("--config")
        .arg(&path)
        .args(["infra", "hosts", "--json", "name"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        json!([{ "name": "test" }])
    );
    std::fs::write(&path, "infrastructure = '../infra'\n").unwrap();
    let output = isolated_command()
        .arg("--config")
        .arg(&path)
        .args(["infra", "hosts", "--json", "name"])
        .current_dir("/")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = isolated_command()
        .arg("--config")
        .arg(&path)
        .args(["config", "unset", "infrastructure"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let output = isolated_command()
        .arg("--config")
        .arg(&path)
        .args(["infra", "hosts"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("collar config set infrastructure"));
}

#[test]
fn rejects_corrupt_missing_and_secret_configuration_without_overwriting() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("config.toml");
    let output = isolated_command()
        .arg("--config")
        .arg(&path)
        .args(["config", "get"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    for text in [
        "token = 'very-secret-value'",
        "host = ['very-secret-value']",
        "host = '",
    ] {
        std::fs::write(&path, text).unwrap();
        let output = isolated_command()
            .arg("--config")
            .arg(&path)
            .args(["config", "set", "host", "git.cmu.dev"])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("very-secret-value"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
    }
    let output = command()
        .args(["config", "set", "host", "git.cmu.dev"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("configuration disabled"));
}

#[test]
fn saved_host_controls_api_and_token_binding_but_not_explicit_other_host() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("config.toml");
    let configured = Server::new(vec![Response::json(json!({"version": "test"}))]);
    std::fs::write(&path, format!("host = '{}'\n", configured.origin)).unwrap();
    let output = isolated_command()
        .arg("--config")
        .arg(&path)
        .env("COLLAR_TOKEN", "test-token")
        .args(["api", "version"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let requests = configured.task.join().unwrap();
    assert!(
        requests[0]
            .to_lowercase()
            .contains("authorization: token test-token")
    );
    let other = Server::new(vec![Response::json(json!({"version": "test"}))]);
    let output = isolated_command()
        .arg("--config")
        .arg(&path)
        .env("COLLAR_TOKEN", "test-token")
        .args(["--host", &other.origin, "api", "version"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let requests = other.task.join().unwrap();
    assert!(!requests[0].to_lowercase().contains("authorization:"));
}

#[test]
fn inferred_remote_overrides_saved_host_without_receiving_its_token() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("config.toml");
    std::fs::write(&path, "host = 'saved.example'\n").unwrap();
    let server = Server::new(vec![Response::json(json!({
        "full_name": "ScottyLabs/test", "description": null,
        "html_url": "https://example.test/test", "default_branch": "main",
        "private": false, "archived": false
    }))]);
    for args in [
        vec!["init", "--quiet"],
        vec![
            "remote",
            "add",
            "origin",
            &format!("{}/ScottyLabs/test.git", server.origin),
        ],
    ] {
        assert!(
            Command::new("git")
                .args(args)
                .current_dir(temp.path())
                .status()
                .unwrap()
                .success()
        );
    }
    let output = isolated_command()
        .arg("--config")
        .arg(&path)
        .env("COLLAR_TOKEN", "saved-host-token")
        .current_dir(temp.path())
        .args(["repo", "view", "--json", "nameWithOwner"])
        .output()
        .unwrap();
    success(&output);
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        json!({"nameWithOwner": "ScottyLabs/test"})
    );
    let requests = server.finish();
    assert!(requests[0].starts_with("GET /api/v1/repos/ScottyLabs/test "));
    assert!(!requests[0].to_lowercase().contains("authorization:"));
}

#[test]
fn explicit_checkout_and_host_environment_override_saved_defaults() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("config.toml");
    std::fs::write(
        &path,
        "host = 'saved.example'\ninfrastructure = 'missing'\n",
    )
    .unwrap();
    let checkout = temp.path().join("infra");
    std::fs::create_dir_all(checkout.join("modules/hosts/test")).unwrap();
    std::fs::create_dir_all(checkout.join("modules/roles")).unwrap();
    std::fs::write(checkout.join("modules/hosts/test/configuration.nix"), "{}").unwrap();
    std::fs::write(checkout.join("modules/roles/test.nix"), "{}").unwrap();
    let output = isolated_command()
        .env("COLLAR_CONFIG", &path)
        .env("COLLAR_INFRASTRUCTURE", &checkout)
        .args(["infra", "hosts", "--json", "name"])
        .output()
        .unwrap();
    success(&output);
    let output = isolated_command()
        .arg("--config")
        .arg(&path)
        .env("COLLAR_INFRASTRUCTURE", "missing")
        .args(["infra", "hosts", "--checkout"])
        .arg(&checkout)
        .args(["--json", "name"])
        .output()
        .unwrap();
    success(&output);
    let server = Server::new(vec![Response::json(json!({"version": "test"}))]);
    let output = isolated_command()
        .arg("--config")
        .arg(&path)
        .env("COLLAR_HOST", &server.origin)
        .env("COLLAR_TOKEN", "env-host-token")
        .args(["api", "version"])
        .output()
        .unwrap();
    success(&output);
    assert!(
        server.finish()[0]
            .to_lowercase()
            .contains("authorization: token env-host-token")
    );
    let original = std::fs::read_to_string(&path).unwrap();
    let output = isolated_command()
        .arg("--config")
        .arg(&path)
        .args(["config", "set", "governance"])
        .arg(&checkout)
        .output()
        .unwrap();
    failure(&output, "checkout is missing data/org.toml");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
}

fn project_governance(checkout: &std::path::Path) {
    std::fs::create_dir_all(checkout.join("data/teams")).unwrap();
    std::fs::write(checkout.join("data/org.toml"), "[org.forgejo]\norg = 'ScottyLabs'\nurl = 'https://git.cmu.dev'\n[org.keycloak]\nurl = 'https://idp.scottylabs.org'\nrealm = 'scottylabs'\n").unwrap();
    std::fs::write(checkout.join("data/teams/team.toml"), "[team]\nslug = 'tech'\nname = 'Tech'\nleads = ['Lead']\n[[team.projects]]\nslug = 'collie'\nname = 'Collie'\nmembers = ['member']\n[[team.projects.repos]]\nname = 'collie-api'\n[team.projects.repos.features]\nkennel = {}\n[[team.projects.repos]]\nname = 'collie-web'\n[team.projects.repos.features]\nsentry = {}\n").unwrap();
}

#[test]
fn project_context_uses_canonical_ownership_and_repository_specific_features() {
    let temp = TempDir::new().unwrap();
    project_governance(temp.path());
    let output = command()
        .args([
            "-R",
            "scottylabs/COLLIE-api",
            "project",
            "view",
            "--checkout",
        ])
        .arg(temp.path())
        .args([
            "--json",
            "slug,team,leads,members,features,secretNamespace,declaredOnly,linkedAccountsUrl",
        ])
        .output()
        .unwrap();
    success(&output);
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        json!({
            "slug": "collie", "team": "tech", "leads": ["Lead"], "members": ["member"],
            "features": {"kennel": {}}, "secretNamespace": "collie", "declaredOnly": true,
            "linkedAccountsUrl": "https://idp.scottylabs.org/realms/scottylabs/account/account-security/linked-accounts"
        })
    );
    for (repo, expected) in [
        ("other.example/ScottyLabs/collie-api", "different forge"),
        ("someone/collie-api", "outside the governance organization"),
        ("ScottyLabs/absent", "not declared in governance"),
    ] {
        let output = command()
            .args(["-R", repo, "project", "view", "--checkout"])
            .arg(temp.path())
            .output()
            .unwrap();
        failure(&output, expected);
        assert!(output.stdout.is_empty());
    }
    let file = temp.path().join("data/teams/team.toml");
    let mut content = std::fs::read_to_string(&file).unwrap();
    content.push_str("\n[[team.projects]]\nname = 'Duplicate'\nslug = 'duplicate'\n[[team.projects.repos]]\nname = 'COLLIE-API'\n");
    std::fs::write(file, content).unwrap();
    let output = command()
        .args([
            "-R",
            "ScottyLabs/collie-api",
            "project",
            "view",
            "--checkout",
        ])
        .arg(temp.path())
        .output()
        .unwrap();
    failure(&output, "ambiguous governance ownership");
}
