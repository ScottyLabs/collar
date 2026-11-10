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

fn label(id: u64, name: &str) -> Value {
    json!({"id":id,"name":name,"color":"aabbcc","description":"","exclusive":false,"is_archived":false,"url":null})
}

fn not_found() -> Response {
    Response {
        status: 404,
        body: "{}".into(),
        headers: String::new(),
    }
}

#[test]
fn issue_create_resolves_organization_labels_to_ids() {
    let mut created = issue(77, false);
    created["labels"] = json!([label(113, "type/bug")]);
    let server = Server::new(vec![
        Response::json(json!([])),
        Response::json(json!([label(113, "type/bug")])),
        Response::json(json!([])),
        Response::json(created),
    ]);
    let output = server
        .command(true)
        .args(["issue", "create", "--title", "A bug", "--label", "type/bug"])
        .args(["--json", "labels"])
        .output()
        .unwrap();
    success(&output);
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap()["labels"][0]["name"],
        "type/bug"
    );
    let requests = server.finish();
    assert!(requests[0].starts_with("GET /api/v1/repos/ScottyLabs/terrier/labels?"));
    assert!(requests[1].starts_with("GET /api/v1/orgs/ScottyLabs/labels?"));
    assert!(requests[3].starts_with("POST /api/v1/repos/ScottyLabs/terrier/issues "));
    assert_eq!(
        request_body(&requests[3]),
        json!({"title":"A bug","body":"","labels":[113]})
    );
}

#[test]
fn unknown_labels_fail_before_listing_or_writing() {
    let server = Server::new(vec![
        Response::json(json!([label(5, "stage/todo")])),
        Response::json(json!([])),
        not_found(),
    ]);
    let output = server
        .command(false)
        .args(["issue", "list", "--label", "stage/tdo"])
        .output()
        .unwrap();
    failure(
        &output,
        "unknown label \"stage/tdo\"; available: stage/todo",
    );
    assert_eq!(server.finish().len(), 3);

    let server = Server::new(vec![]);
    let output = server
        .command(false)
        .args(["issue", "list", "--label", "a,b"])
        .output()
        .unwrap();
    failure(&output, "cannot contain commas");
    assert!(server.finish().is_empty());
}

#[test]
fn issue_list_sends_validated_label_filter() {
    let server = Server::new(vec![
        Response::json(json!([])),
        Response::json(json!([label(113, "type/bug"), label(120, "urgency/high")])),
        Response::json(json!([])),
        Response::json(json!([issue(42, false)])),
    ]);
    let output = server
        .command(false)
        .args(["issue", "list", "--limit", "1", "--label", "type/bug"])
        .args(["--label", "urgency/high", "--json", "number"])
        .output()
        .unwrap();
    success(&output);
    let requests = server.finish();
    assert!(requests[3].starts_with(
        "GET /api/v1/repos/ScottyLabs/terrier/issues?state=open&type=issues&labels=type%2Fbug%2Curgency%2Fhigh&page=1"
    ));
}

#[test]
fn issue_edit_replaces_labels_in_one_write() {
    let mut current = issue(54, false);
    current["labels"] = json!([label(5, "stage/todo"), label(113, "type/bug")]);
    let server = Server::with_responses(|_| {
        vec![
            Response::json(current.clone()),
            Response::json(json!([
                label(5, "stage/todo"),
                label(6, "stage/in-progress")
            ])),
            Response::json(json!([])),
            not_found(),
            Response::json(json!([
                label(113, "type/bug"),
                label(6, "stage/in-progress")
            ])),
        ]
    });
    let output = server
        .command(true)
        .args(["issue", "edit", "54", "--add-label", "stage/in-progress"])
        .args(["--remove-label", "stage/todo", "--json", "labels"])
        .output()
        .unwrap();
    success(&output);
    let names: Vec<_> = serde_json::from_slice::<Value>(&output.stdout).unwrap()["labels"]
        .as_array()
        .unwrap()
        .iter()
        .map(|label| label["name"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(names, ["type/bug", "stage/in-progress"]);
    let requests = server.finish();
    assert_eq!(requests.len(), 5);
    assert!(requests[4].starts_with("PUT /api/v1/repos/ScottyLabs/terrier/issues/54/labels "));
    assert_eq!(request_body(&requests[4]), json!({"labels":[113,6]}));
}

#[test]
fn issue_edit_skips_unchanged_labels_and_reports_partial_writes() {
    let mut current = issue(54, false);
    current["labels"] = json!([label(5, "stage/todo")]);
    let server = Server::new(vec![
        Response::json(current.clone()),
        Response::json(json!([label(5, "stage/todo")])),
        Response::json(json!([])),
        not_found(),
    ]);
    let output = server
        .command(true)
        .args(["issue", "edit", "54", "--add-label", "stage/todo"])
        .output()
        .unwrap();
    success(&output);
    assert_eq!(
        server.finish().len(),
        4,
        "no write when labels are unchanged"
    );

    let server = Server::new(vec![
        Response::json(current.clone()),
        Response::json(json!([label(5, "stage/todo"), label(6, "stage/done")])),
        Response::json(json!([])),
        not_found(),
        Response::json(current),
        Response {
            status: 422,
            body: "{}".into(),
            headers: String::new(),
        },
    ]);
    let output = server
        .command(true)
        .args(["issue", "edit", "54", "--title", "Renamed"])
        .args(["--add-label", "stage/done"])
        .output()
        .unwrap();
    failure(
        &output,
        "the title or body was updated, but the labels were not",
    );
    let requests = server.finish();
    assert!(requests[4].starts_with("PATCH "));
    assert!(requests[5].starts_with("PUT "));

    let server = Server::new(vec![]);
    let output = server
        .command(true)
        .args([
            "issue",
            "edit",
            "54",
            "--add-label",
            "x",
            "--remove-label",
            "x",
        ])
        .output()
        .unwrap();
    failure(&output, "x");
}
