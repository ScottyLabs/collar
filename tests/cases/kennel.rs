use super::*;

const HEAD: &str = "abcdef0123456789abcdef0123456789abcdef01";

fn status(id: u64, context: &str, state: &str) -> Value {
    json!({
        "id": id,
        "context": context,
        "status": state,
        "description": format!("{context} {state}"),
        "target_url": format!("https://nixbot.example/{context}")
    })
}

fn branch(request: Value) -> Response {
    Response::json(json!({
        "project": "terrier",
        "branch": "pr-42",
        "request": request,
        "deployments": [{
            "id": "d1",
            "name": "web",
            "kind": "site",
            "commit": HEAD,
            "url": "https://terrier-web-pr-42.scottylabs.net",
            "custom_url": null,
            "health": null
        }]
    }))
}

fn request(status: &str, stage: Option<&str>) -> Value {
    json!({"commit": HEAD, "status": status, "failure_stage": stage})
}

fn kennel(server: &Server, token: bool) -> std::process::Command {
    let mut command = server.command(token);
    command.env("COLLAR_KENNEL", &server.origin);
    command
}

#[test]
fn preview_reads_the_pr_branch_from_kennel() {
    let server = Server::new(vec![
        Response::json(pull(HEAD)),
        branch(request("deployed", None)),
    ]);
    let output = kennel(&server, false)
        .args(["kennel", "preview", "42", "--json"])
        .output()
        .unwrap();
    success(&output);
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        value["deployments"][0]["url"],
        "https://terrier-web-pr-42.scottylabs.net"
    );
    let requests = server.finish();
    assert!(requests[1].starts_with("GET /projects/terrier/branches/pr-42 HTTP/1.1"));
}

#[test]
fn kennel_output_field_errors_precede_network_access() {
    let output = command()
        .args([
            "--host",
            "http://127.0.0.1:1",
            "-R",
            "ScottyLabs/terrier",
            "kennel",
            "preview",
            "42",
            "--json",
            "nope",
        ])
        .output()
        .unwrap();
    failure(&output, "unsupported JSON field: nope");
}

#[test]
fn logs_fetches_failed_nixbot_logs_without_the_token_and_reports_the_deploy() {
    let server = Server::with_responses(|origin| {
        let mut failed = status(3, "nixbot/nix-build", "failure");
        failed["target_url"] = json!(format!("{origin}/repos/gitea/ScottyLabs/terrier/builds/7"));
        vec![
            Response::json(pull(HEAD)),
            Response::json(json!([failed, status(1, "kennel/deploy", "success")])),
            Response::json(json!([])),
            branch(request("failed", Some("healthcheck"))),
            Response::json(json!({
                "status": "failed",
                "error": null,
                "failures": [{
                    "attr": "checks.x86_64-linux.kennel",
                    "status": "failed",
                    "error": null,
                    "log_tail": "error: builder failed"
                }]
            })),
        ]
    });
    let output = kennel(&server, true)
        .args(["kennel", "logs", "42", "--tail", "5", "--json"])
        .output()
        .unwrap();
    success(&output);
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["statuses"].as_array().unwrap().len(), 1);
    assert_eq!(
        value["failures"][0]["failures"][0]["logTail"],
        "error: builder failed"
    );
    assert_eq!(value["request"]["failure_stage"], "healthcheck");
    let requests = server.finish();
    assert!(
        requests[4].starts_with(
            "GET /api/repos/gitea/ScottyLabs/terrier/builds/7/failures?tail=5 HTTP/1.1"
        )
    );
    assert!(!requests[3].contains("test-secret"));
    assert!(!requests[4].contains("test-secret"));
}

#[test]
fn wait_succeeds_once_the_build_passes_and_kennel_deployed_the_head() {
    let server = Server::new(vec![
        Response::json(pull(HEAD)),
        Response::json(pull(HEAD)),
        Response::json(json!([status(1, "nixbot/nix-build", "success")])),
        Response::json(json!([])),
        branch(request("deployed", None)),
        Response::json(pull(HEAD)),
    ]);
    let output = kennel(&server, false)
        .args(["kennel", "wait", "42", "--timeout", "1", "--json"])
        .output()
        .unwrap();
    success(&output);
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["build"]["status"], "success");
    assert_eq!(value["deploy"]["status"], "deployed");
    server.finish();
}

#[test]
fn wait_fails_on_eval_or_deploy_failure_head_change_and_closed_pr() {
    let eval = Server::new(vec![
        Response::json(pull(HEAD)),
        Response::json(pull(HEAD)),
        Response::json(json!([status(1, "nixbot/nix-eval", "failure")])),
        Response::json(json!([])),
    ]);
    let output = kennel(&eval, false)
        .args(["kennel", "wait", "42", "--timeout", "1"])
        .output()
        .unwrap();
    failure(&output, "nixbot evaluation failed");
    eval.finish();

    let deploy = Server::new(vec![
        Response::json(pull(HEAD)),
        Response::json(pull(HEAD)),
        Response::json(json!([status(1, "nixbot/nix-build", "success")])),
        Response::json(json!([])),
        branch(request("failed", Some("healthcheck"))),
    ]);
    let output = kennel(&deploy, false)
        .args(["kennel", "wait", "42", "--timeout", "1"])
        .output()
        .unwrap();
    failure(&output, "Kennel deployment failed");
    failure(&output, "healthcheck");
    deploy.finish();

    let changed = Server::new(vec![
        Response::json(pull(HEAD)),
        Response::json(pull("new-head")),
    ]);
    let output = kennel(&changed, false)
        .args(["kennel", "wait", "42", "--timeout", "1"])
        .output()
        .unwrap();
    failure(&output, "PR head changed while waiting");
    changed.finish();

    let mut closed_pull = pull(HEAD);
    closed_pull["state"] = json!("closed");
    let closed = Server::new(vec![
        Response::json(pull(HEAD)),
        Response::json(closed_pull),
    ]);
    let output = kennel(&closed, false)
        .args(["kennel", "wait", "42", "--timeout", "1"])
        .output()
        .unwrap();
    failure(&output, "closed while waiting");
    closed.finish();
}

#[test]
fn wait_times_out_while_nothing_is_reported() {
    let server = Server::new(vec![
        Response::json(pull(HEAD)),
        Response::json(pull(HEAD)),
        Response::json(json!([])),
        branch(Value::Null),
    ]);
    let output = kennel(&server, false)
        .args(["kennel", "wait", "42", "--timeout", "1"])
        .output()
        .unwrap();
    failure(&output, "timed out waiting for the nixbot build");
    server.finish();
}

#[test]
fn status_combines_nixbot_checks_with_kennel_deployments() {
    let server = Server::new(vec![
        Response::json(pull(HEAD)),
        Response::json(json!([status(1, "nixbot/nix-eval", "success")])),
        Response::json(json!([])),
        branch(request("deployed", None)),
    ]);
    let output = kennel(&server, false)
        .args(["kennel", "status", "42", "--json"])
        .output()
        .unwrap();
    success(&output);
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["statuses"][0]["context"], "nixbot/nix-eval");
    assert_eq!(report["request"]["status"], "deployed");
    assert_eq!(report["deployments"][0]["name"], "web");
    server.finish();
}

#[test]
fn deployment_log_lines_reject_wrong_identity_before_authentication() {
    for value in [
        json!({"project":"other","branch":"pr-42","request":request("deployed", None),"deployments":[]}),
        json!({"project":"terrier","branch":"pr-43","request":request("deployed", None),"deployments":[]}),
        json!({"project":"terrier","branch":"pr-42","request":{"commit":"b".repeat(40),"status":"deployed","failure_stage":null},"deployments":[]}),
    ] {
        let server = Server::new(vec![Response::json(pull(HEAD)), Response::json(value)]);
        let output = kennel(&server, false)
            .args(["kennel", "logs", "42", "--lines", "--json"])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert_eq!(server.finish().len(), 2);
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(
            error.contains("different project or branch")
                || error.contains("no request or deployment"),
            "{error}"
        );
    }
}
