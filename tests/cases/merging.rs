use super::*;

fn request_body(request: &str) -> Value {
    serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap()
}

#[test]
fn merge_posts_inspected_head_and_exact_options() {
    let server = Server::new(vec![
        Response::json(pull("merge-head")),
        Response {
            status: 200,
            body: String::new(),
            headers: String::new(),
        },
    ]);
    let output = server
        .command(true)
        .args([
            "pr",
            "merge",
            "42",
            "--squash",
            "--match-head-commit",
            "merge-head",
            "--delete-branch",
            "--json",
        ])
        .output()
        .unwrap();
    success(&output);
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        json!({"number":42,"headRefOid":"merge-head","method":"squash","autoMergeRequested":false,"accepted":true})
    );
    let requests = server.finish();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].starts_with("GET /api/v1/repos/ScottyLabs/terrier/pulls/42 "));
    assert!(requests[1].starts_with("POST /api/v1/repos/ScottyLabs/terrier/pulls/42/merge "));
    assert_eq!(
        request_body(&requests[1]),
        json!({"Do":"squash","head_commit_id":"merge-head","delete_branch_after_merge":true,"force_merge":false,"merge_when_checks_succeed":false})
    );
}

#[test]
fn update_branch_posts_style_without_body() {
    let server = Server::new(vec![
        Response::json(pull("update-head")),
        Response {
            status: 200,
            body: String::new(),
            headers: String::new(),
        },
    ]);
    let output = server
        .command(true)
        .args([
            "pr",
            "update-branch",
            "42",
            "--rebase",
            "--match-head-commit",
            "update-head",
            "--json",
        ])
        .output()
        .unwrap();
    success(&output);
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        json!({"number":42,"headRefOid":"update-head","method":"rebase","accepted":true})
    );
    let requests = server.finish();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].starts_with("GET /api/v1/repos/ScottyLabs/terrier/pulls/42 "));
    assert!(
        requests[1]
            .starts_with("POST /api/v1/repos/ScottyLabs/terrier/pulls/42/update?style=rebase ")
    );
    assert!(requests[1].ends_with("\r\n\r\n"));
}

#[test]
fn merge_and_update_refuse_changed_or_unmergeable_pull_heads() {
    let mut closed = pull("head");
    closed["state"] = json!("closed");
    for (args, response, message) in [
        (
            vec![
                "pr",
                "merge",
                "42",
                "--merge",
                "--match-head-commit",
                "expected",
            ],
            pull("actual"),
            "PR head differs",
        ),
        (
            vec!["pr", "update-branch", "42"],
            closed,
            "PR must be open and unmerged",
        ),
    ] {
        let server = Server::new(vec![Response::json(response)]);
        let output = server.command(true).args(args).output().unwrap();
        failure(&output, message);
        assert_eq!(server.finish().len(), 1);
    }
}

#[test]
fn merge_requires_exactly_one_strategy_before_network() {
    for args in [
        vec!["pr", "merge", "42"],
        vec!["pr", "merge", "42", "--merge", "--squash"],
        vec![
            "pr",
            "merge",
            "42",
            "--merge",
            "--auto",
            "--match-head-commit",
            "head",
        ],
    ] {
        let output = command()
            .args(["-R", "ScottyLabs/terrier"])
            .args(args)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn scheduled_merge_accepts_created_response_and_reports_unpinned_queue() {
    let server = Server::new(vec![
        Response::json(pull("head")),
        Response {
            status: 201,
            body: String::new(),
            headers: String::new(),
        },
    ]);
    let output = server
        .command(true)
        .args(["pr", "merge", "42", "--merge", "--auto"])
        .output()
        .unwrap();
    success(&output);
    assert!(String::from_utf8_lossy(&output.stdout).contains("without a pinned head"));
    let requests = server.finish();
    assert_eq!(
        request_body(&requests[1])["merge_when_checks_succeed"],
        true
    );
    assert_eq!(request_body(&requests[1])["force_merge"], false);
}

#[test]
fn invalid_json_fields_and_missing_token_fail_before_network() {
    let server = Server::new(vec![]);
    let invalid = server
        .command(true)
        .args(["pr", "merge", "42", "--merge", "--json", "number,unknown"])
        .output()
        .unwrap();
    failure(&invalid, "unknown");
    let unauthenticated = server
        .command(false)
        .args(["pr", "update-branch", "42"])
        .output()
        .unwrap();
    failure(&unauthenticated, "COLLAR_TOKEN");
    assert!(server.finish().is_empty());
}

#[test]
fn merge_conflict_is_not_retried_or_echoed() {
    let server = Server::new(vec![
        Response::json(pull("head")),
        Response {
            status: 409,
            body: "head conflict: secret details".into(),
            headers: String::new(),
        },
    ]);
    let output = server
        .command(true)
        .args(["pr", "merge", "42", "--rebase"])
        .output()
        .unwrap();
    failure(&output, "409");
    let requests = server.finish();
    assert_eq!(requests.len(), 2);
    assert!(requests[1].starts_with("POST /api/v1/repos/ScottyLabs/terrier/pulls/42/merge "));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("secret details"));
}
