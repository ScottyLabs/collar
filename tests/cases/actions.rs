use super::*;

fn run(id: u64, status: &str) -> Value {
    json!({
        "id": id,
        "index_in_repo": id,
        "workflow_id": "ci.yml",
        "status": status,
        "event": "push",
        "title": "CI",
        "commit_sha": format!("sha-{id}"),
        "prettyref": "refs/heads/main",
        "html_url": format!("https://git.cmu.dev/ScottyLabs/terrier/actions/runs/{id}")
    })
}

#[test]
fn run_list_reads_forgejo_wrapper_and_paginates_with_native_query_fields() {
    let server = Server::new(vec![
        Response::json(json!({"total_count": 2, "workflow_runs": [run(7, "success")]})),
        Response::json(json!({"total_count": 2, "workflow_runs": [run(8, "failure")]})),
    ]);
    let output = server
        .command(false)
        .args([
            "actions",
            "list",
            "--status",
            "failure",
            "--ref",
            "refs/heads/main",
            "--limit",
            "2",
            "--json",
        ])
        .output()
        .unwrap();
    success(&output);
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value[0]["id"], 7);
    assert_eq!(value[1]["id"], 8);
    let requests = server.finish();
    assert!(requests[0].starts_with("GET /api/v1/repos/ScottyLabs/terrier/actions/runs?status=failure&ref=refs%2Fheads%2Fmain&page=1&limit=50"));
    assert!(requests[1].contains("page=2"));
}

#[test]
fn run_list_rejects_unrecognized_status_before_network_access() {
    let output = command()
        .args([
            "--host",
            "http://127.0.0.1:1",
            "-R",
            "ScottyLabs/terrier",
            "actions",
            "list",
            "--status",
            "pending",
        ])
        .output()
        .unwrap();
    failure(&output, "unsupported workflow run status: pending");
}

#[test]
fn run_list_rejects_incomplete_records_instead_of_returning_partial_success() {
    let mut incomplete = run(7, "success");
    incomplete.as_object_mut().unwrap().remove("commit_sha");
    let server = Server::new(vec![Response::json(json!({
        "total_count": 1,
        "workflow_runs": [incomplete]
    }))]);
    let output = server
        .command(false)
        .args(["actions", "list"])
        .output()
        .unwrap();
    failure(&output, "Forgejo action run has no commit_sha");
    server.finish();
}

#[test]
fn jobs_and_run_view_use_documented_forgejo_endpoints() {
    let server = Server::new(vec![
        Response::json(
            json!({"id": 12, "status": "running", "commit_sha": "abcdef", "title": "CI"}),
        ),
        Response::json(
            json!([{"id": 90, "run_id": 12, "name": "test", "status": "success", "attempt": 1}]),
        ),
    ]);
    let output = server
        .command(false)
        .args(["actions", "view", "12", "--json"])
        .output()
        .unwrap();
    success(&output);
    let output = server
        .command(false)
        .args(["actions", "jobs", "12", "--json"])
        .output()
        .unwrap();
    success(&output);
    let requests = server.finish();
    assert!(requests[0].starts_with("GET /api/v1/repos/ScottyLabs/terrier/actions/runs/12 "));
    assert!(requests[1].starts_with("GET /api/v1/repos/ScottyLabs/terrier/actions/runs/12/jobs "));
}

#[test]
fn deleting_a_run_requires_authentication_before_network_access() {
    let output = command()
        .args([
            "--host",
            "http://127.0.0.1:1",
            "-R",
            "ScottyLabs/terrier",
            "actions",
            "delete",
            "12",
        ])
        .output()
        .unwrap();
    failure(&output, "requires collar auth login or COLLAR_TOKEN");
}

#[test]
fn dispatch_uses_forgejo_payload_names_and_reports_empty_success() {
    let server = Server::new(vec![Response {
        status: 204,
        body: String::new(),
        headers: String::new(),
    }]);
    let output = server
        .command(true)
        .args([
            "workflow",
            "dispatch",
            "ci.yml",
            "--ref",
            "main",
            "--field",
            "mode=fast",
            "--field",
            "enabled=yes",
        ])
        .args(["--json"])
        .output()
        .unwrap();
    success(&output);
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        json!({"dispatched": true})
    );
    let request = &server.finish()[0];
    assert!(
        request.starts_with(
            "POST /api/v1/repos/ScottyLabs/terrier/actions/workflows/ci.yml/dispatches "
        )
    );
    assert!(request.contains(r#""ref":"main""#));
    assert!(request.contains(r#""inputs":{"enabled":"yes","mode":"fast"}"#));
    assert!(request.contains(r#""return_run_info":false"#));
}

#[test]
fn workflow_inputs_are_validated_before_network_access() {
    let mut command = command();
    command
        .env("COLLAR_TOKEN", "test-secret")
        .env("COLLAR_HOST", "http://127.0.0.1:1");
    let output = command
        .args([
            "--host",
            "http://127.0.0.1:1",
            "-R",
            "ScottyLabs/terrier",
            "workflow",
            "dispatch",
            "ci.yml",
            "--ref",
            "main",
            "--field",
            "invalid",
        ])
        .output()
        .unwrap();
    failure(&output, "workflow input must use NAME=VALUE");
}

#[test]
fn workflow_dispatch_requires_token() {
    let output = command()
        .args([
            "--host",
            "http://127.0.0.1:1",
            "-R",
            "ScottyLabs/terrier",
            "workflow",
            "dispatch",
            "ci.yml",
            "--ref",
            "main",
            "--field",
            "invalid",
        ])
        .output()
        .unwrap();
    failure(&output, "requires collar auth login or COLLAR_TOKEN");
}

#[test]
fn workflow_list_ignores_only_missing_directories_after_repository_is_verified() {
    let server = Server::new(vec![
        Response::json(json!({"id": 1, "full_name": "ScottyLabs/terrier"})),
        Response {
            status: 404,
            body: "{}".into(),
            headers: String::new(),
        },
        Response {
            status: 404,
            body: "{}".into(),
            headers: String::new(),
        },
    ]);
    let output = server
        .command(false)
        .args(["workflow", "list", "--json"])
        .output()
        .unwrap();
    success(&output);
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        json!([])
    );
    assert_eq!(server.finish().len(), 3);

    let missing = Server::new(vec![Response {
        status: 404,
        body: "{}".into(),
        headers: String::new(),
    }]);
    let output = missing
        .command(false)
        .args(["workflow", "list"])
        .output()
        .unwrap();
    failure(
        &output,
        "verify repository before reading workflow directories",
    );
    assert_eq!(missing.finish().len(), 1);
}

#[test]
fn workflow_dispatch_rejects_non_object_success_response() {
    let server = Server::new(vec![Response {
        status: 201,
        body: "null".into(),
        headers: String::new(),
    }]);
    let output = server
        .command(true)
        .args(["workflow", "dispatch", "ci.yml", "--ref", "main"])
        .output()
        .unwrap();
    failure(
        &output,
        "Forgejo workflow dispatch response must be an object",
    );
    server.finish();
}

#[test]
fn workflow_path_rejects_traversal_without_network_access() {
    let output = command()
        .args([
            "--host",
            "http://127.0.0.1:1",
            "-R",
            "ScottyLabs/terrier",
            "workflow",
            "view",
            "../secrets.yml",
        ])
        .output()
        .unwrap();
    failure(
        &output,
        "workflow must be a filename or path under a workflow directory",
    );
}
