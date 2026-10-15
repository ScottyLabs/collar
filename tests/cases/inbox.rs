use super::*;

fn item(number: u64, is_pull: bool) -> Value {
    json!({
        "number": number,
        "title": format!("Task {number}"),
        "html_url": format!("https://git.cmu.dev/ScottyLabs/collar/issues/{number}"),
        "updated_at": "2026-10-15T12:00:00Z",
        "repository": {"full_name": "ScottyLabs/collar"},
        "pull_request": if is_pull { json!({"draft": true}) } else { Value::Null }
    })
}

#[test]
fn inbox_uses_authenticated_server_filters_without_a_checkout() {
    let directory = TempDir::new().unwrap();
    let server = Server::new(vec![
        Response::json(json!([item(1, true)])),
        Response::json(json!([item(2, true)])),
        Response::json(json!([item(3, false)])),
    ]);
    let output = command()
        .current_dir(directory.path())
        .env("COLLAR_HOST", &server.origin)
        .env("COLLAR_TOKEN", "test-secret")
        .args(["status", "--owner", "ScottyLabs", "--limit", "5", "--json"])
        .output()
        .unwrap();
    success(&output);
    let inbox: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(inbox["reviewRequests"][0]["number"], 1);
    assert_eq!(inbox["authoredPullRequests"][0]["isDraft"], true);
    assert_eq!(
        inbox["assignedIssues"][0]["repository"],
        "ScottyLabs/collar"
    );
    let requests = server.finish();
    assert_eq!(requests.len(), 3);
    for (request, filter) in requests
        .iter()
        .zip(["review_requested", "created", "assigned"])
    {
        assert!(request.starts_with("GET /api/v1/repos/issues/search?state=open&"));
        assert!(request.contains(&format!("{filter}=true")));
        assert!(request.contains("sort=recentupdate&page=1&limit=5&owner=ScottyLabs"));
        assert!(
            request
                .to_lowercase()
                .contains("authorization: token test-secret")
        );
    }
}

#[test]
fn inbox_validates_output_scope_and_authentication_before_network_access() {
    for (args, error) in [
        (vec!["status", "--json", "typo"], "unsupported JSON field"),
        (vec!["status"], "requires collar auth login"),
        (vec!["status", "-R", "terrier"], "use --owner instead of -R"),
    ] {
        let output = command()
            .args(["--host", "http://127.0.0.1:1"])
            .args(args)
            .output()
            .unwrap();
        failure(&output, error);
    }
}

#[test]
fn inbox_does_not_hide_api_failures_as_empty_categories() {
    let server = Server::new(vec![Response {
        status: 403,
        body: "denied".into(),
        headers: String::new(),
    }]);
    let output = command()
        .env("COLLAR_HOST", &server.origin)
        .env("COLLAR_TOKEN", "test-secret")
        .args(["status", "--json"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(server.finish().len(), 1);
}
