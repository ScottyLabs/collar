use super::*;

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
fn revalidated_unchanged_page_does_not_hide_new_subsequent_pages() {
    let server = Server::new(vec![
        Response {
            status: 200,
            body: json!([pull("one")]).to_string(),
            headers: "ETag: \"one\"\r\nLink: <http://example.invalid/?page=1>; rel=\"prev\"\r\n"
                .into(),
        },
        Response::json(json!([])),
        Response {
            status: 304,
            body: String::new(),
            headers: String::new(),
        },
        Response {
            status: 200,
            body: json!([pull("two")]).to_string(),
            headers: "Link: <http://example.invalid/?page=1>; rel=\"prev\"\r\n".into(),
        },
        Response::json(json!([])),
    ]);
    success(
        &server
            .command(false)
            .args(["pr", "list", "--json"])
            .output()
            .unwrap(),
    );
    let second = server
        .command(false)
        .args(["pr", "list", "--json"])
        .output()
        .unwrap();
    success(&second);
    assert_eq!(
        serde_json::from_slice::<Value>(&second.stdout)
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let requests = server.finish();
    assert_eq!(requests.len(), 5);
    assert!(
        requests[2]
            .to_lowercase()
            .contains("if-none-match: \"one\"")
    );
    assert!(requests[3].contains("page=2"));
}

#[test]
fn primed_cache_cannot_bypass_the_review_head_recheck() {
    let server = Server::new(vec![
        Response {
            status: 200,
            body: pull("before").to_string(),
            headers: "ETag: \"before\"\r\n".into(),
        },
        Response {
            status: 304,
            body: String::new(),
            headers: String::new(),
        },
        Response {
            status: 200,
            body: pull("after").to_string(),
            headers: "ETag: \"after\"\r\n".into(),
        },
    ]);
    success(
        &server
            .command(true)
            .args(["pr", "view", "42", "--json"])
            .output()
            .unwrap(),
    );
    let output = server
        .command(true)
        .args(["pr", "review", "42", "--approve"])
        .output()
        .unwrap();
    failure(&output, "head changed");
    let requests = server.finish();
    assert_eq!(requests.len(), 3);
    assert!(requests.iter().all(|request| request.starts_with("GET ")));
    assert!(
        requests[1]
            .to_lowercase()
            .contains("if-none-match: \"before\"")
    );
}

#[test]
fn previous_page_links_do_not_prove_a_listing_is_complete() {
    let server = Server::new(vec![
        Response {
            status: 200,
            body: json!([pull("one")]).to_string(),
            headers: "Link: <https://other.example/?page=1>; rel=\"prev\"\r\n".into(),
        },
        Response::json(json!([pull("two")])),
        Response::json(json!([])),
    ]);
    let output = server
        .command(false)
        .args(["pr", "list", "--json"])
        .output()
        .unwrap();
    success(&output);
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout)
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let requests = server.finish();
    assert_eq!(requests.len(), 3);
    assert!(requests[1].contains("page=2"));
}
