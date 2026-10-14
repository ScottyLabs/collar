use super::*;

fn issue(number: u64, pull_request: bool) -> Value {
    let mut value = json!({
        "number": number,
        "title": format!("Issue {number}"),
        "body": "body",
        "state": "open",
        "html_url": format!("https://git.cmu.dev/ScottyLabs/terrier/issues/{number}"),
        "user": { "login": "contributor" },
        "labels": [],
        "comments": 0
    });
    if pull_request {
        value["pull_request"] =
            json!({"html_url":"https://git.cmu.dev/ScottyLabs/terrier/pulls/42"});
    }
    value
}

fn request_body(request: &str) -> Value {
    serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap()
}

#[test]
fn issue_list_uses_native_type_filter_and_applies_limit() {
    let server = Server::new(vec![
        Response::json(json!([issue(42, false)])),
        Response::json(json!([issue(43, false)])),
    ]);
    let output = server
        .command(false)
        .args(["issue", "list", "--limit", "2", "--json", "number,title"])
        .output()
        .unwrap();
    success(&output);
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        json!([
            {"number":42,"title":"Issue 42"}, {"number":43,"title":"Issue 43"}
        ])
    );
    let requests = server.finish();
    assert!(requests[0].starts_with(
        "GET /api/v1/repos/ScottyLabs/terrier/issues?state=open&type=issues&page=1&limit=50 "
    ));
    assert!(requests[1].contains("page=2"));
}

#[test]
fn issue_list_rejects_pull_requests_even_when_type_filter_is_set() {
    let server = Server::new(vec![
        Response::json(json!([issue(42, true)])),
        Response::json(json!([])),
    ]);
    let output = server
        .command(false)
        .args(["issue", "list"])
        .output()
        .unwrap();
    failure(&output, "pull request for type=issues");
    assert!(server.finish()[0].contains("type=issues"));
}

#[test]
fn issue_mutations_refuse_pull_requests_before_posting() {
    let server = Server::new(vec![Response::json(issue(42, true))]);
    let output = server
        .command(true)
        .args(["issue", "close", "42"])
        .output()
        .unwrap();
    failure(&output, "not an issue");
    let requests = server.finish();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].starts_with("GET /api/v1/repos/ScottyLabs/terrier/issues/42 "));
}

#[test]
fn issue_body_file_is_sent_verbatim_and_requires_auth() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("body.md");
    let body = "First line.\n\nLiteral $value and `code`.\n";
    std::fs::write(&path, body).unwrap();
    let server = Server::new(vec![Response::json(issue(77, false))]);
    let output = server
        .command(true)
        .args(["issue", "create", "--title", "A task", "--body-file"])
        .arg(&path)
        .output()
        .unwrap();
    success(&output);
    let requests = server.finish();
    assert!(requests[0].starts_with("POST /api/v1/repos/ScottyLabs/terrier/issues "));
    assert_eq!(
        request_body(&requests[0]),
        json!({"title":"A task","body":body})
    );
    assert!(
        requests[0]
            .to_lowercase()
            .contains("authorization: token test-secret")
    );

    let server = Server::new(vec![]);
    let output = server
        .command(false)
        .args(["issue", "create", "--title", "A task"])
        .output()
        .unwrap();
    failure(&output, "requires collar auth login");
    assert!(server.finish().is_empty());
}

#[test]
fn label_mutations_use_native_payloads_and_explicit_delete() {
    let label = json!({"id":9,"name":"good first issue","color":"aabbcc","description":"starter","exclusive":false,"is_archived":false,"url":"https://git.cmu.dev/api/v1/repos/ScottyLabs/terrier/labels/9"});
    let server = Server::new(vec![Response::json(label.clone())]);
    let output = server
        .command(true)
        .args([
            "label",
            "create",
            "--name",
            "good first issue",
            "--color",
            "aabbcc",
            "--description",
            "starter",
            "--json",
            "id,name",
        ])
        .output()
        .unwrap();
    success(&output);
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        json!({"id":9,"name":"good first issue"})
    );
    let requests = server.finish();
    assert!(requests[0].starts_with("POST /api/v1/repos/ScottyLabs/terrier/labels "));
    assert_eq!(
        request_body(&requests[0]),
        json!({"name":"good first issue","color":"aabbcc","description":"starter"})
    );

    let server = Server::new(vec![Response {
        status: 204,
        body: String::new(),
        headers: String::new(),
    }]);
    let output = server
        .command(true)
        .args(["label", "delete", "9"])
        .output()
        .unwrap();
    success(&output);
    assert!(server.finish()[0].starts_with("DELETE /api/v1/repos/ScottyLabs/terrier/labels/9 "));
}
