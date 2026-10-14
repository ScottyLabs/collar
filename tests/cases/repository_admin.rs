use super::*;

fn body(request: &str) -> Value {
    serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap()
}

#[test]
fn create_selects_native_org_endpoint_and_sends_requested_fields() {
    let server = Server::new(vec![Response::json(json!({
        "full_name":"ScottyLabs/new",
        "html_url":"https://git.cmu.dev/ScottyLabs/new"
    }))]);
    let output = server
        .command(true)
        .args([
            "repo",
            "admin",
            "create",
            "new",
            "--owner",
            "ScottyLabs",
            "--owner-type",
            "org",
            "--description",
            "New project",
            "--private",
        ])
        .output()
        .unwrap();
    success(&output);
    let requests = server.finish();
    assert!(requests[0].starts_with("POST /api/v1/orgs/ScottyLabs/repos "));
    assert_eq!(
        body(&requests[0]),
        json!({"name":"new","description":"New project","private":true})
    );
}

#[test]
fn create_reports_incomplete_success_response_without_retrying() {
    let server = Server::new(vec![Response::json(json!({"full_name":"ScottyLabs/new"}))]);
    let output = server
        .command(true)
        .args([
            "repo",
            "admin",
            "create",
            "new",
            "--owner",
            "ScottyLabs",
            "--owner-type",
            "org",
        ])
        .output()
        .unwrap();
    failure(&output, "omitted repository html_url");
    let requests = server.finish();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].starts_with("POST /api/v1/orgs/ScottyLabs/repos "));
}

#[test]
fn personal_repository_creation_checks_authenticated_owner() {
    let server = Server::new(vec![Response::json(json!({"login":"owner"}))]);
    let output = server
        .command(true)
        .args([
            "repo",
            "admin",
            "create",
            "new",
            "--owner",
            "someone-else",
            "--owner-type",
            "user",
        ])
        .output()
        .unwrap();
    failure(&output, "must match the authenticated account");
    let requests = server.finish();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].starts_with("GET /api/v1/user "));
}

#[test]
fn edit_can_explicitly_make_repository_public() {
    let server = Server::new(vec![Response::json(json!({
        "full_name":"ScottyLabs/terrier",
        "html_url":"https://git.cmu.dev/ScottyLabs/terrier",
        "private":false
    }))]);
    let output = server
        .command(true)
        .args([
            "repo",
            "admin",
            "edit",
            "--public",
            "-R",
            "ScottyLabs/terrier",
        ])
        .output()
        .unwrap();
    success(&output);
    let requests = server.finish();
    assert!(requests[0].starts_with("PATCH /api/v1/repos/ScottyLabs/terrier "));
    assert_eq!(body(&requests[0]), json!({"private":false}));
}

#[test]
fn delete_requires_exact_confirmation_before_request() {
    let server = Server::new(vec![]);
    let output = server
        .command(true)
        .args([
            "repo",
            "admin",
            "delete",
            "--confirm",
            "ScottyLabs/other",
            "-R",
            "ScottyLabs/terrier",
        ])
        .output()
        .unwrap();
    failure(&output, "exactly match");
    assert!(server.finish().is_empty());
}

#[test]
fn mirror_sync_checks_repository_kind_before_posting() {
    let server = Server::new(vec![Response::json(json!({"mirror":false}))]);
    let output = server
        .command(true)
        .args(["repo", "admin", "mirror-sync", "-R", "ScottyLabs/terrier"])
        .output()
        .unwrap();
    failure(&output, "not a mirror");
    let requests = server.finish();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].starts_with("GET /api/v1/repos/ScottyLabs/terrier "));
}
