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
        .env_remove("COLLAR_CACHE_DIR")
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

#[test]
fn repository_fork_creates_a_copy_with_native_options() {
    let server = Server::new(vec![Response::json(json!({
        "full_name": "contributor/collar-copy",
        "html_url": "https://git.cmu.dev/contributor/collar-copy"
    }))]);
    let output = server
        .command(true)
        .args([
            "repo",
            "fork",
            "-R",
            "ScottyLabs/collar",
            "--name",
            "collar-copy",
        ])
        .output()
        .unwrap();
    success(&output);
    let requests = server.finish();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].starts_with("POST /api/v1/repos/ScottyLabs/collar/forks "));
    let body: Value = serde_json::from_str(requests[0].split_once("\r\n\r\n").unwrap().1).unwrap();
    assert_eq!(body, json!({"name": "collar-copy"}));
}

#[test]
fn provisioned_configuration_commands_are_not_exposed() {
    for arguments in [
        vec!["repo", "admin", "create", "scratch"],
        vec!["governance", "changes", "join"],
        vec!["kennel", "docs", "list"],
        vec!["governance", "run"],
        vec!["governance", "validate"],
        vec!["infra", "colmena"],
        vec!["changelog", "new"],
    ] {
        let output = command()
            .args(["--host", "http://127.0.0.1:1"])
            .args(arguments)
            .output()
            .unwrap();
        failure(&output, "unrecognized subcommand");
        assert_eq!(output.status.code(), Some(2));
    }
}

#[test]
fn interactive_commands_reject_headless_use_before_network_access() {
    for arguments in [vec!["project", "select"], vec!["pr", "browse"]] {
        let output = command()
            .args(["--host", "http://127.0.0.1:1", "-R", "ScottyLabs/terrier"])
            .args(arguments)
            .output()
            .unwrap();
        failure(&output, "requires terminal stdin and stderr");
        assert!(output.stdout.is_empty());
    }
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
        Self::with_responses(|_| responses)
    }

    fn with_responses(responses: impl FnOnce(&str) -> Vec<Response>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let responses = responses(&origin);
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
            "--inline-comment",
            "src/main.rs",
            "1",
            "new",
            "Explain this",
        ])
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
    let server = Server::new(vec![
        Response::json(pull("head")),
        Response { status: 200, body: "diff --git a/src/main.rs b/src/main.rs\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1 +1 @@\n-old\n+new\n".into(), headers: String::new() },
    ]);
    let output = server
        .command(true)
        .args([
            "pr",
            "review",
            "42",
            "--comment",
            "--inline-comment",
            "src/main.rs",
            "99",
            "new",
            "Outside the diff",
        ])
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
    failure(&output, "invalid governance team");
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
    let requests = server.finish();
    assert_eq!(requests.len(), 4);
    assert!(
        requests[3].starts_with("GET /api/v1/repos/ScottyLabs/terrier/issues/42/comments HTTP/1.1")
    );
}

#[test]
fn saves_and_unsets_host_defaults_atomically() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("settings/config.toml");
    let output = isolated_command()
        .arg("--config")
        .arg(&path)
        .args(["config", "set", "host", "example.test"])
        .output()
        .unwrap();
    success(&output);
    let output = isolated_command()
        .arg("--config")
        .arg(&path)
        .args(["config", "get", "--json", "host"])
        .output()
        .unwrap();
    success(&output);
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        json!({"host":"https://example.test/"})
    );
    let output = isolated_command()
        .arg("--config")
        .arg(&path)
        .args(["config", "unset", "host"])
        .output()
        .unwrap();
    success(&output);
    let output = isolated_command()
        .arg("--config")
        .arg(&path)
        .args(["config", "get", "--json", "host"])
        .output()
        .unwrap();
    success(&output);
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        json!({"host":null})
    );
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
fn explicit_checkouts_and_host_environment_are_respected() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("config.toml");
    std::fs::write(&path, "host = 'saved.example'\n").unwrap();
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
    failure(&output, "invalid value 'governance'");
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

#[cfg(unix)]
fn doctor_fixture(temp: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf) {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let bin = temp.join("bin");
    std::fs::create_dir(&bin).unwrap();
    let git = std::env::split_paths(&std::env::var_os("PATH").unwrap())
        .map(|directory| directory.join("git"))
        .find(|path| path.is_file())
        .unwrap();
    symlink(git, bin.join("git")).unwrap();
    for tool in ["nix", "devenv", "bao", "secretspec", "ssh-add"] {
        let file = bin.join(tool);
        std::fs::write(&file, format!("#!/bin/sh\nprintf '%s %s\\n' '{tool}' \"$*\" >> \"$COLLAR_TEST_LOG\"\nprintf '%s\\n' 'never-print-tool-secret'\nexit 0\n")).unwrap();
        std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let project = temp.join("project");
    let governance = temp.join("governance");
    let infrastructure = temp.join("infrastructure");
    std::fs::create_dir(&project).unwrap();
    for file in ["devenv.nix", "devenv.yaml", "flake.nix"] {
        std::fs::write(project.join(file), "{}").unwrap();
    }
    std::fs::write(
        project.join("secretspec.toml"),
        "[project]\nname = 'collie'\nrevision = '1.0'\n[profiles.dev]\n",
    )
    .unwrap();
    project_governance(&governance);
    std::fs::create_dir_all(infrastructure.join("modules/hosts/test")).unwrap();
    std::fs::create_dir_all(infrastructure.join("modules/roles")).unwrap();
    std::fs::write(
        infrastructure.join("modules/hosts/test/configuration.nix"),
        "{}",
    )
    .unwrap();
    std::fs::write(infrastructure.join("modules/roles/test.nix"), "{}").unwrap();
    for checkout in [&project, &governance, &infrastructure] {
        for args in [
            vec!["init", "--quiet", "--initial-branch=main"],
            vec!["config", "core.hooksPath", "/dev/null"],
            vec!["config", "commit.gpgsign", "false"],
            vec!["config", "user.name", "Test"],
            vec!["config", "user.email", "test@example.test"],
            vec!["add", "."],
            vec!["commit", "--quiet", "-m", "test"],
            vec!["config", "branch.main.remote", "."],
            vec!["config", "branch.main.merge", "refs/heads/main"],
        ] {
            assert!(
                Command::new("git")
                    .args(args)
                    .current_dir(checkout)
                    .status()
                    .unwrap()
                    .success()
            );
        }
    }
    std::fs::write(temp.join("config.toml"), "").unwrap();
    (project, bin)
}

#[cfg(unix)]
fn doctor_command(
    temp: &std::path::Path,
    project: &std::path::Path,
    bin: &std::path::Path,
) -> Command {
    let mut command = isolated_command();
    command
        .arg("--config")
        .arg(temp.join("config.toml"))
        .args(["-R", "ScottyLabs/collie-api", "doctor", "--json"])
        .env("PATH", bin)
        .env("COLLAR_TEST_LOG", temp.join("commands.log"))
        .arg("--governance")
        .arg(temp.join("governance"))
        .arg("--infrastructure")
        .arg(temp.join("infrastructure"))
        .current_dir(project);
    command
}

#[cfg(unix)]
#[test]
fn offline_doctor_reports_manual_checks_and_never_resolves_secrets() {
    let temp = TempDir::new().unwrap();
    let (project, bin) = doctor_fixture(temp.path());
    let output = doctor_command(temp.path(), &project, &bin)
        .arg("--offline")
        .output()
        .unwrap();
    success(&output);
    let checks: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert!(checks.iter().all(|check| check["status"] != "fail"));
    for name in ["forge", "forge-auth", "bao-auth"] {
        assert!(
            checks
                .iter()
                .any(|check| check["name"] == name && check["status"] == "skipped")
        );
    }
    assert!(
        checks
            .iter()
            .any(|check| check["name"] == "linked-accounts" && check["status"] == "manual")
    );
    let commands = std::fs::read_to_string(temp.path().join("commands.log")).unwrap();
    assert!(!commands.contains("token lookup"));
    assert!(!commands.contains("secretspec check"));
    assert!(!String::from_utf8_lossy(&output.stdout).contains("never-print-tool-secret"));
    std::fs::write(
        project.join("secretspec.toml"),
        "[project]\nname = 'collie-api'\nrevision = '1.0'\n[profiles.dev]\n",
    )
    .unwrap();
    std::fs::write(
        temp.path().join("governance/data/teams/extra.toml"),
        "broken = [",
    )
    .unwrap();
    let output = doctor_command(temp.path(), &project, &bin)
        .arg("--offline")
        .output()
        .unwrap();
    failure(&output, "diagnostics failed");
    let checks: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        checks
            .iter()
            .any(|check| check["name"] == "governance" && check["status"] == "fail")
    );
    assert!(
        checks
            .iter()
            .any(|check| check["name"] == "governance-revision" && check["status"] == "fail")
    );
    assert!(
        checks
            .iter()
            .any(|check| check["name"] == "secretspec" && check["status"] == "skipped")
    );
}

#[cfg(unix)]
#[test]
fn online_doctor_verifies_forge_and_bao_without_echoing_tool_output() {
    let temp = TempDir::new().unwrap();
    let (project, bin) = doctor_fixture(temp.path());
    let server = Server::new(vec![
        Response::json(json!({"version": "16.0.5"})),
        Response::json(json!({"login": "member"})),
    ]);
    let org_path = temp.path().join("governance/data/org.toml");
    let org = std::fs::read_to_string(&org_path)
        .unwrap()
        .replace("https://git.cmu.dev", &server.origin);
    std::fs::write(org_path, org).unwrap();
    let output = doctor_command(temp.path(), &project, &bin)
        .env("COLLAR_HOST", &server.origin)
        .env("COLLAR_TOKEN", "test-token")
        .output()
        .unwrap();
    failure(&output, "diagnostics failed");
    let checks: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    for name in ["forge", "forge-auth", "bao-auth"] {
        assert!(
            checks
                .iter()
                .any(|check| check["name"] == name && check["status"] == "pass")
        );
    }
    assert!(
        checks
            .iter()
            .any(|check| check["name"] == "governance-revision" && check["status"] == "fail")
    );
    let requests = server.finish();
    assert!(requests[0].starts_with("GET /api/v1/version "));
    assert!(requests[1].starts_with("GET /api/v1/user "));
    let commands = std::fs::read_to_string(temp.path().join("commands.log")).unwrap();
    assert!(commands.contains("bao token lookup -format=json"));
    assert!(!commands.contains("secretspec check"));
    assert!(!String::from_utf8_lossy(&output.stdout).contains("never-print-tool-secret"));
}

#[cfg(unix)]
#[test]
fn doctor_aggregates_tool_namespace_and_profile_failures() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempDir::new().unwrap();
    let (project, bin) = doctor_fixture(temp.path());
    std::fs::remove_file(bin.join("nix")).unwrap();
    std::fs::write(
        bin.join("bao"),
        "#!/bin/sh\nprintf 'never-print-tool-secret' >&2\nexit 7\n",
    )
    .unwrap();
    std::fs::set_permissions(bin.join("bao"), std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::write(
        project.join("secretspec.toml"),
        "[project]\nname = 'collie-api'\nrevision = '1.0'\n[profiles.dev]\n",
    )
    .unwrap();
    let output = doctor_command(temp.path(), &project, &bin)
        .arg("--offline")
        .output()
        .unwrap();
    failure(&output, "diagnostics failed");
    let checks: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    for name in ["tool:nix", "tool:bao", "secretspec"] {
        assert!(
            checks
                .iter()
                .any(|check| check["name"] == name && check["status"] == "fail")
        );
    }
    assert!(!String::from_utf8_lossy(&output.stdout).contains("never-print-tool-secret"));
    std::fs::write(
        project.join("secretspec.toml"),
        "[project]\nname = 'collie'\nrevision = '1.0'\n[profiles.dev]\n",
    )
    .unwrap();
    let output = doctor_command(temp.path(), &project, &bin)
        .args(["--offline", "--profile", "prod"])
        .output()
        .unwrap();
    failure(&output, "diagnostics failed");
    let checks: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert!(checks.iter().any(|check| check["name"] == "secretspec"
        && check["detail"].as_str().unwrap().contains("profile prod")));
}

const SOURCE_REVISION: &str = "1111111111111111111111111111111111111111";

fn source_head() -> Vec<Response> {
    vec![
        Response::json(json!({"default_branch": "main"})),
        Response::json(json!({"commit": {"id": SOURCE_REVISION}})),
    ]
}

#[test]
fn changelog_uses_latest_pinned_snapshots_and_explicit_offline_cache() {
    let temp = TempDir::new().unwrap();
    let files = [
        ("governance.md", "# Governance\n"),
        ("infrastructure.md", "# Infrastructure\n"),
        ("kennel.md", "# Kennel\n"),
        (
            "updates/017-current.md",
            "# Current\n\nNo action required.\n",
        ),
    ];
    let mut responses = source_head();
    responses.push(Response::json(
        json!({"page":1,"total_count":4,"truncated":false,
        "tree":files.iter().map(|(path, body)| tree_entry(path, body)).collect::<Vec<_>>()}),
    ));
    responses.extend(files.iter().map(|(_, body)| Response {
        status: 200,
        body: body.to_string(),
        headers: String::new(),
    }));
    let server = Server::new(responses);
    let mut cmd = command();
    cmd.args(["--host", &server.origin, "--cache-dir"])
        .arg(temp.path())
        .args(["changelog", "update", "17", "--json"]);
    let output = cmd.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["title"], "Current");
    assert_eq!(value["dataSource"]["repository"], "ScottyLabs/changelog");
    assert_eq!(value["dataSource"]["revision"], SOURCE_REVISION);
    let requests = server.finish();
    assert!(requests[0].contains("/repos/ScottyLabs/changelog "));
    assert!(
        requests[3..]
            .iter()
            .all(|request| request.contains(SOURCE_REVISION))
    );
    let output = cmd.arg("--offline").output().unwrap();
    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["dataSource"]["offline"], true);
}

fn tree_entry(path: &str, content: &str) -> Value {
    use sha1::Digest;
    let sha = format!(
        "{:x}",
        sha1::Sha1::digest(format!("blob {}\0{content}", content.len()).as_bytes())
    );
    json!({"path": path, "type": "blob", "mode": "100644", "size": content.len(), "sha": sha})
}

#[test]
fn canonical_governance_snapshots_paginate_pin_files_and_require_explicit_offline() {
    let temp = TempDir::new().unwrap();
    let server = Server::with_responses(|origin| {
        let org = format!(
            "[org.forgejo]\norg = 'ScottyLabs'\nurl = '{origin}'\n[org.keycloak]\nurl = 'https://idp.scottylabs.org'\nrealm = 'scottylabs'\n"
        );
        let team =
            "[team]\nslug = 'collie'\nname = 'Collie'\n[[team.repos]]\nname = 'collie-api'\n";
        let mut responses = source_head();
        responses.extend([
            Response::json(json!({"page": 1, "total_count": 2, "truncated": true, "tree": [tree_entry("data/org.toml", &org)]})),
            Response::json(json!({"page": 2, "total_count": 2, "truncated": false, "tree": [tree_entry("data/teams/collie.toml", team)]})),
            Response {status: 200, body: org, headers: String::new()},
            Response {status: 200, body: team.into(), headers: String::new()},
            Response {status: 503, body: "never-print-server-secret".into(), headers: String::new()},
        ]);
        responses
    });
    let run = |offline| {
        let mut cmd = command();
        cmd.args(["--host", &server.origin, "--cache-dir"])
            .arg(temp.path())
            .args([
                "governance",
                "project",
                "view",
                "collie-api",
                "--json",
                "slug,secretNamespace,dataSource",
            ]);
        if offline {
            cmd.arg("--offline");
        }
        cmd.output().unwrap()
    };
    let output = run(false);
    success(&output);
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["slug"], "collie");
    assert_eq!(value["secretNamespace"], "collie");
    assert_eq!(value["dataSource"]["revision"], SOURCE_REVISION);
    assert_eq!(value["dataSource"]["offline"], false);
    success(&run(true));
    let foreign = command()
        .args(["--host", "other.example", "--cache-dir"])
        .arg(temp.path())
        .args(["governance", "project", "list", "--offline", "--json"])
        .output()
        .unwrap();
    failure(&foreign, "no readable cached governance snapshot");
    let project = command()
        .args(["--host", &server.origin, "--cache-dir"])
        .arg(temp.path())
        .args([
            "-R",
            "ScottyLabs/collie-api",
            "project",
            "view",
            "--offline",
            "--json",
            "slug,dataSource",
        ])
        .output()
        .unwrap();
    success(&project);

    let failed = run(false);
    failure(&failed, "HTTP 503");
    assert!(failed.stdout.is_empty());
    let cached = run(true);
    success(&cached);
    let value: Value = serde_json::from_slice(&cached.stdout).unwrap();
    assert_eq!(value["dataSource"]["offline"], true);
    let requests = server.finish();
    assert_eq!(requests.len(), 7);
    assert!(requests[2].contains("recursive=true&page=1&per_page=1000"));
    assert!(requests[3].contains("page=2"));
    for request in &requests[4..6] {
        assert!(request.contains(&format!("ref={SOURCE_REVISION}")));
    }
    let mut source = command();
    source
        .args([
            "--host",
            value["dataSource"]["host"].as_str().unwrap(),
            "--cache-dir",
        ])
        .arg(temp.path())
        .args([
            "governance",
            "project",
            "view",
            "collie-api",
            "--offline",
            "--json",
            "source",
        ]);
    let output = source.output().unwrap();
    success(&output);
    let path: Value = serde_json::from_slice(&output.stdout).unwrap();
    let file = std::path::Path::new(path["source"].as_str().unwrap());
    let content = std::fs::read_to_string(file).unwrap();
    std::fs::write(file, content.replace("Collie", "Tamper")).unwrap();
    failure(
        &source.output().unwrap(),
        "does not match its pinned Git blob",
    );
    std::fs::remove_file(path["source"].as_str().unwrap()).unwrap();
    failure(&source.output().unwrap(), "cached source is incomplete");
}

#[test]
fn canonical_infrastructure_snapshots_expose_source_inventory_without_git() {
    let temp = TempDir::new().unwrap();
    let files = [
        ("flake.nix", "{}"),
        ("modules/hosts/infra/configuration.nix", "{}"),
        ("modules/hosts/infra/kennel.nix", "{}"),
        ("modules/roles/infra.nix", "{}"),
    ];
    let mut responses = source_head();
    responses.push(Response::json(json!({"page": 1, "total_count": files.len(), "truncated": false, "tree": files.iter().map(|(path, content)| tree_entry(path,content)).collect::<Vec<_>>()})));
    responses.extend(files.iter().map(|(_, content)| Response {
        status: 200,
        body: (*content).into(),
        headers: String::new(),
    }));
    let server = Server::new(responses);
    let output = command()
        .args(["--host", &server.origin, "--cache-dir"])
        .arg(temp.path())
        .args(["infra", "service", "kennel", "--json", "host,dataSource"])
        .output()
        .unwrap();
    success(&output);
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value[0]["host"], "infra");
    assert_eq!(
        value[0]["dataSource"]["repository"],
        "ScottyLabs/infrastructure"
    );
    let output = command()
        .args(["--host", &server.origin, "--cache-dir"])
        .arg(temp.path())
        .args(["infra", "hosts", "--offline", "--json", "name,dataSource"])
        .output()
        .unwrap();
    success(&output);
    assert_eq!(server.finish().len(), 7);
}

#[test]
fn source_snapshots_reject_unsafe_incomplete_and_symlink_trees_before_raw_reads() {
    for (entry, total, message) in [
        (
            json!({"path":"data/teams/../../escape", "type":"blob", "mode":"100644", "size":2, "sha":SOURCE_REVISION}),
            1,
            "unsafe path",
        ),
        (
            json!({"path":"data/org.toml", "type":"blob", "mode":"120000", "size":2, "sha":SOURCE_REVISION}),
            1,
            "symlinks",
        ),
        (
            json!({"path":"data/org.toml", "type":"blob", "mode":"100644", "size":2, "sha":SOURCE_REVISION}),
            2,
            "incomplete",
        ),
        (
            json!({"path":"data/org.toml", "type":"blob", "mode":"100644", "size":u64::MAX, "sha":SOURCE_REVISION}),
            1,
            "exceeds 8 MiB",
        ),
    ] {
        let temp = TempDir::new().unwrap();
        let mut responses = source_head();
        responses.push(Response::json(
            json!({"page":1,"total_count":total,"truncated":false,"tree":[entry]}),
        ));
        let server = Server::new(responses);
        let output = command()
            .args(["--host", &server.origin, "--cache-dir"])
            .arg(temp.path())
            .args(["governance", "project", "list", "--json"])
            .output()
            .unwrap();
        failure(&output, message);
        assert!(output.stdout.is_empty());
        assert_eq!(server.finish().len(), 3);
    }
}

#[test]
fn failed_snapshot_download_does_not_publish_an_offline_source() {
    let temp = TempDir::new().unwrap();
    let mut responses = source_head();
    responses.extend([
        Response::json(json!({"page":1,"total_count":1,"truncated":false,"tree":[tree_entry("data/org.toml", "{}")]})),
        Response {status:403,body:"never-print-server-secret".into(),headers:String::new()},
    ]);
    let server = Server::new(responses);
    let mut cmd = command();
    cmd.args(["--host", &server.origin, "--cache-dir"])
        .arg(temp.path())
        .args(["governance", "project", "list", "--json"]);
    let output = cmd.output().unwrap();
    failure(&output, "HTTP 403");
    assert!(!String::from_utf8_lossy(&output.stderr).contains("never-print-server-secret"));
    let output = cmd.arg("--offline").output().unwrap();
    failure(&output, "no readable cached governance snapshot");
    assert_eq!(server.finish().len(), 4);
}

#[cfg(unix)]
#[path = "cases/environment.rs"]
mod environment;

#[path = "cases/authoring.rs"]
mod authoring;

#[path = "cases/merging.rs"]
mod merging;

#[path = "cases/checkout.rs"]
mod checkout;

#[path = "cases/auth.rs"]
mod auth;

#[cfg(unix)]
#[path = "cases/infra_ops.rs"]
mod infra_ops;

#[cfg(unix)]
#[path = "cases/governance_ops.rs"]
mod governance_ops;

#[path = "cases/kennel.rs"]
mod kennel;

#[path = "cases/issues.rs"]
mod issues;

#[path = "cases/releases.rs"]
mod releases;

#[path = "cases/actions.rs"]
mod actions;

#[path = "cases/clone.rs"]
mod clone;

#[test]
fn changelog_reads_numbered_announcements_and_rejects_ambiguity() {
    let temp = TempDir::new().unwrap();
    std::fs::create_dir(temp.path().join("updates")).unwrap();
    for (path, body) in [
        ("updates/001-first.md", "# First\n\nNo action required.\n"),
        (
            "updates/017-current.md",
            "# Current\n\nAction required if using previews.\n",
        ),
        ("updates/README.md", "not an announcement"),
        ("kennel.md", "# Kennel\n\n## 2026-10-14\n\nChanges\n"),
    ] {
        std::fs::write(temp.path().join(path), body).unwrap();
    }
    let output = command()
        .args(["changelog", "updates", "--checkout"])
        .arg(temp.path())
        .args(["--json", "number,title"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        json!([
            {"number":17,"title":"Current"},{"number":1,"title":"First"}
        ])
    );
    let output = command()
        .args(["changelog", "update", "17", "--checkout"])
        .arg(temp.path())
        .args(["--json", "body"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("Action required"));
    let output = command()
        .args(["changelog", "view", "kennel", "--checkout"])
        .arg(temp.path())
        .args(["--json", "title"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        json!({"title":"Kennel"})
    );
    std::fs::write(temp.path().join("updates/017-duplicate.md"), "# Duplicate").unwrap();
    let output = command()
        .args(["changelog", "update", "17", "--checkout"])
        .arg(temp.path())
        .output()
        .unwrap();
    failure(&output, "duplicate announcement number");
}

#[cfg(unix)]
#[path = "cases/oauth.rs"]
mod oauth;

#[path = "cases/docs.rs"]
mod docs;

#[cfg(unix)]
#[test]
fn closed_output_pipe_terminates_without_a_panic() {
    use std::os::unix::process::ExitStatusExt;
    use std::process::Stdio;

    let mut child = command()
        .args(["completion", "bash"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.signal(), Some(13));
    assert!(output.stderr.is_empty());
}
