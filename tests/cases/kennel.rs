use super::*;

fn deployment_comment(sha: &str) -> Value {
    json!({
        "body": format!("<!-- kennel-deployment -->\n### Kennel Deployments\n\n| Service | URL |\n| --- | --- |\n| app | https://terrier-app-pr-42.scottylabs.net |\n\nLast updated for commit `{}`.\n", &sha[..7]),
        "html_url": "https://git.cmu.dev/ScottyLabs/terrier/issues/42#issuecomment-2"
    })
}

fn status(id: u64, context: &str, state: &str) -> Value {
    json!({
        "id": id,
        "context": context,
        "status": state,
        "description": format!("{context} {state}"),
        "target_url": format!("https://logs.scottylabs.net/{context}")
    })
}

#[test]
fn preview_reads_canonical_marked_comment_and_marks_its_revision() {
    let head = "abcdef0123456789abcdef0123456789abcdef01";
    let server = Server::new(vec![
        Response::json(pull(head)),
        Response::json(json!([deployment_comment(head)])),
    ]);
    let output = server
        .command(false)
        .args(["kennel", "preview", "42", "--json"])
        .output()
        .unwrap();
    success(&output);
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["comments"][0]["revision"], "matches-head-prefix");
    assert_eq!(
        value["comments"][0]["previewUrls"],
        json!(["https://terrier-app-pr-42.scottylabs.net/"])
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("currentness"));
    let requests = server.finish();
    assert!(
        requests[1].starts_with("GET /api/v1/repos/ScottyLabs/terrier/issues/42/comments HTTP/1.1")
    );
}

#[test]
fn preview_unknown_revision_and_rejects_urls_with_credentials() {
    let head = "abcdef0123456789abcdef0123456789abcdef01";
    let mut comment = deployment_comment(head);
    comment["body"] = json!(
        "<!-- kennel-deployment -->\n| app | https://user:secret@example.test/path |\nLast updated for commit ``.\n"
    );
    let server = Server::new(vec![
        Response::json(pull(head)),
        Response::json(json!([comment])),
    ]);
    let output = server
        .command(false)
        .args(["kennel", "preview", "42", "--json"])
        .output()
        .unwrap();
    success(&output);
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["comments"][0]["revision"], "unknown");
    assert_eq!(value["comments"][0]["previewUrls"], json!([]));
    server.finish();
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
fn logs_reads_status_field_named_status_and_prints_report_links() {
    let head = "abcdef0123456789abcdef0123456789abcdef01";
    let server = Server::new(vec![
        Response::json(pull(head)),
        Response::json(json!([status(2, "kennel/build", "success")])),
        Response::json(json!([status(1, "kennel/build", "pending")])),
        Response::json(json!([])),
    ]);
    let output = server
        .command(false)
        .args(["kennel", "logs", "42", "--json"])
        .output()
        .unwrap();
    success(&output);
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["statuses"][0]["status"], "success");
    assert_eq!(
        value["statuses"][0]["targetUrl"],
        "https://logs.scottylabs.net/kennel/build"
    );
    assert!(value["note"].as_str().unwrap().contains("Historical"));
    let requests = server.finish();
    assert!(requests[1].starts_with("GET /api/v1/repos/ScottyLabs/terrier/statuses/abcdef0123456789abcdef0123456789abcdef01?page=1"));
}

#[test]
fn wait_succeeds_only_for_both_reports_then_rechecks_pr_head() {
    let head = "abcdef0123456789abcdef0123456789abcdef01";
    let server = Server::new(vec![
        Response::json(pull(head)),
        Response::json(pull(head)),
        Response::json(json!([
            status(1, "kennel/build", "success"),
            status(2, "kennel/deploy", "success")
        ])),
        Response::json(json!([])),
        Response::json(pull(head)),
    ]);
    let output = server
        .command(false)
        .args(["kennel", "wait", "42", "--timeout", "1", "--json"])
        .output()
        .unwrap();
    success(&output);
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["build"]["status"], "success");
    assert_eq!(value["deploy"]["status"], "success");
    assert!(value["health"].as_str().unwrap().contains("not checked"));
    server.finish();
}

#[test]
fn wait_fails_for_build_failure_head_change_and_closed_pr() {
    let head = "abcdef0123456789abcdef0123456789abcdef01";
    let failed = Server::new(vec![
        Response::json(pull(head)),
        Response::json(pull(head)),
        Response::json(json!([status(1, "kennel/build", "failure")])),
        Response::json(json!([])),
    ]);
    let output = failed
        .command(false)
        .args(["kennel", "wait", "42", "--timeout", "1"])
        .output()
        .unwrap();
    failure(&output, "Kennel build failed");
    failed.finish();

    let changed = Server::new(vec![
        Response::json(pull(head)),
        Response::json(pull("new-head")),
    ]);
    let output = changed
        .command(false)
        .args(["kennel", "wait", "42", "--timeout", "1"])
        .output()
        .unwrap();
    failure(&output, "PR head changed while waiting");
    changed.finish();

    let mut closed_pull = pull(head);
    closed_pull["state"] = json!("closed");
    let closed = Server::new(vec![
        Response::json(pull(head)),
        Response::json(closed_pull),
    ]);
    let output = closed
        .command(false)
        .args(["kennel", "wait", "42", "--timeout", "1"])
        .output()
        .unwrap();
    failure(&output, "closed while waiting");
    closed.finish();
}

#[test]
fn wait_times_out_when_reports_are_missing() {
    let head = "abcdef0123456789abcdef0123456789abcdef01";
    let server = Server::new(vec![
        Response::json(pull(head)),
        Response::json(pull(head)),
        Response::json(json!([])),
    ]);
    let output = server
        .command(false)
        .args(["kennel", "wait", "42", "--timeout", "1"])
        .output()
        .unwrap();
    failure(&output, "timed out waiting for Kennel");
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
