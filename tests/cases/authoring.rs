use super::*;

fn request_body(request: &str) -> Value {
    serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap()
}

#[test]
fn create_uses_current_branch_and_preserves_markdown_file() {
    let temp = TempDir::new().unwrap();
    assert!(
        git_command()
            .args(["init", "-q", "-b", "feature/authoring"])
            .arg(temp.path())
            .status()
            .unwrap()
            .success()
    );
    let body = "First paragraph.\n\n`literal $value` and \"quotes\".\n";
    std::fs::write(temp.path().join("description.md"), body).unwrap();
    let server = Server::new(vec![Response::json(pull("created"))]);
    let output = server
        .command(true)
        .current_dir(temp.path())
        .args([
            "pr",
            "create",
            "--title",
            "Authoring",
            "--base",
            "main",
            "--body-file",
            "description.md",
            "--json",
            "number",
        ])
        .output()
        .unwrap();
    success(&output);
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        json!({"number":42})
    );
    let requests = server.finish();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        request_body(&requests[0]),
        json!({"title":"Authoring","head":"feature/authoring","base":"main","body":body})
    );
}

#[test]
fn edit_reads_exact_markdown_from_stdin() {
    let server = Server::new(vec![Response::json(pull("edited"))]);
    let mut child = server
        .command(true)
        .args(["pr", "edit", "42", "--body-file", "-"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let body = "Details\n\n- One\n- Two\n";
    child
        .stdin
        .take()
        .unwrap()
        .write_all(body.as_bytes())
        .unwrap();
    success(&child.wait_with_output().unwrap());
    let requests = server.finish();
    assert_eq!(request_body(&requests[0]), json!({"body":body}));
}

#[test]
fn create_posts_exact_fields_and_uses_default_branch_when_omitted() {
    let server = Server::new(vec![
        Response::json(json!({ "default_branch": "trunk" })),
        Response::json(pull("created")),
    ]);
    let output = server
        .command(true)
        .args([
            "pr", "create", "--title", "New PR", "--head", "feat/new", "--body", "Details",
        ])
        .output()
        .unwrap();
    success(&output);
    let requests = server.finish();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].starts_with("GET /api/v1/repos/ScottyLabs/terrier "));
    assert!(requests[1].starts_with("POST /api/v1/repos/ScottyLabs/terrier/pulls "));
    assert_eq!(
        request_body(&requests[1]),
        json!({"title":"New PR","head":"feat/new","base":"trunk","body":"Details"})
    );
}

#[test]
fn edit_sends_only_supplied_fields_and_preserves_empty_body() {
    let server = Server::new(vec![Response::json(pull("edited"))]);
    let output = server
        .command(true)
        .args(["pr", "edit", "42", "--body", "", "--base", "release"])
        .output()
        .unwrap();
    success(&output);
    let requests = server.finish();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].starts_with("PATCH /api/v1/repos/ScottyLabs/terrier/pulls/42 "));
    assert_eq!(
        request_body(&requests[0]),
        json!({"base":"release","body":""})
    );
}

#[test]
fn comment_checks_pr_and_returns_typed_comment_fields() {
    let server = Server::new(vec![
        Response::json(pull("head")),
        Response::json(
            json!({"id":9,"body":"Looks good","html_url":"https://git.cmu.dev/comment/9","user":{"login":"reviewer"}}),
        ),
    ]);
    let output = server
        .command(true)
        .args(["pr", "comment", "42", "--body", "Looks good", "--json"])
        .output()
        .unwrap();
    success(&output);
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        json!({"id":9,"body":"Looks good","url":"https://git.cmu.dev/comment/9","author":{"login":"reviewer"}})
    );
    let requests = server.finish();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].starts_with("GET /api/v1/repos/ScottyLabs/terrier/pulls/42 "));
    assert!(requests[1].starts_with("POST /api/v1/repos/ScottyLabs/terrier/issues/42/comments "));
    assert_eq!(request_body(&requests[1]), json!({"body":"Looks good"}));
}

#[test]
fn close_and_reopen_patch_only_state_after_fetching_pull() {
    for (command_name, initial, expected) in
        [("close", "open", "closed"), ("reopen", "closed", "open")]
    {
        let mut current = pull("head");
        current["state"] = json!(initial);
        let mut changed = current.clone();
        changed["state"] = json!(expected);
        let server = Server::new(vec![Response::json(current), Response::json(changed)]);
        let output = server
            .command(true)
            .args(["pr", command_name, "42", "--json"])
            .output()
            .unwrap();
        success(&output);
        let requests = server.finish();
        assert_eq!(requests.len(), 2);
        assert!(requests[0].starts_with("GET /api/v1/repos/ScottyLabs/terrier/pulls/42 "));
        assert!(requests[1].starts_with("PATCH /api/v1/repos/ScottyLabs/terrier/pulls/42 "));
        assert_eq!(request_body(&requests[1]), json!({"state":expected}));
    }
}

#[test]
fn rejects_missing_auth_blank_values_and_empty_edits_before_network() {
    let server = Server::new(vec![]);
    for (arguments, token, message) in [
        (
            vec!["pr", "create", "--title", "Title", "--head", "branch"],
            false,
            "COLLAR_TOKEN",
        ),
        (
            vec![
                "pr", "create", "--title", "Title", "--head", "branch", "--base", " ",
            ],
            true,
            "base must be nonempty",
        ),
        (
            vec!["pr", "create", "--title", " ", "--head", "branch"],
            true,
            "title must be nonempty",
        ),
        (vec!["pr", "edit", "42"], true, "provide --title"),
        (
            vec!["pr", "comment", "42", "--body", "  "],
            true,
            "comment body must be nonempty",
        ),
    ] {
        let output = server.command(token).args(arguments).output().unwrap();
        failure(&output, message);
    }
    assert!(server.finish().is_empty());
}

#[test]
fn refuses_merged_or_already_closed_pull_without_patch() {
    for (current, message) in [
        (
            {
                let mut p = pull("head");
                p["merged"] = json!(true);
                p
            },
            "merged PR cannot",
        ),
        (
            {
                let mut p = pull("head");
                p["state"] = json!("closed");
                p
            },
            "PR is already closed",
        ),
    ] {
        let server = Server::new(vec![Response::json(current)]);
        let output = server
            .command(true)
            .args(["pr", "close", "42"])
            .output()
            .unwrap();
        failure(&output, message);
        assert_eq!(server.finish().len(), 1);
    }
}

#[test]
fn mutation_forbidden_is_not_retried_or_echoed() {
    let server = Server::new(vec![Response {
        status: 403,
        body: "token test-secret denied".into(),
        headers: String::new(),
    }]);
    let output = server
        .command(true)
        .args(["pr", "edit", "42", "--title", "Updated"])
        .output()
        .unwrap();
    failure(&output, "403");
    let requests = server.finish();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].starts_with("PATCH /api/v1/repos/ScottyLabs/terrier/pulls/42 "));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("denied"));
}

#[test]
fn pull_list_uses_native_author_and_branch_filters_without_a_checkout() {
    let server = Server::new(vec![
        Response::json(json!({"login": "reviewer"})),
        Response::json(json!([pull("head")])),
    ]);
    let output = server
        .command(true)
        .args([
            "pr", "list", "--author", "@me", "--base", "main", "--head", "feature", "--limit", "1",
            "--json",
        ])
        .output()
        .unwrap();
    success(&output);
    let requests = server.finish();
    assert!(requests[0].starts_with("GET /api/v1/user "));
    assert!(
        requests[1].contains("state=open&poster=reviewer&base=main&head=feature&page=1&limit=50")
    );
}

#[test]
fn compact_conversation_paginates_and_preserves_context_without_metadata() {
    for compact in [false, true] {
        let server = Server::new(vec![
            Response::json(pull("head")),
            Response::json(
                json!([{"id":1,"body":"First","user":{"login":"alice","avatar_url":"opaque"},"provider":"opaque"}]),
            ),
            Response::json(
                json!([{"id":2,"body":"Later discussion","html_url":"discussion-url","created_at":"today","user":{"login":"bob","id":99}}]),
            ),
            Response::json(json!([])),
            Response::json(
                json!([{"id":3,"state":"COMMENT","body":"Review","comments_count":2,"provider":"opaque","user":{"login":"reviewer","avatar_url":"opaque"}}]),
            ),
            Response::json(json!([])),
            Response::json(json!([{"id":4,"path":"src/main.rs","body":"First inline"}])),
            Response::json(
                json!([{"id":5,"path":"src/lib.rs","body":"Later inline","diff_hunk":"@@ -1 +2 @@","line":2,"old_line_num":1,"new_line_num":2,"side":"RIGHT","start_line":1,"original_position":7,"html_url":"inline-url","updated_at":"today","user":{"login":"reviewer","avatar_url":"opaque"},"resolver":{"login":"maintainer","avatar_url":"opaque"},"provider":"opaque"}]),
            ),
            Response::json(json!([])),
        ]);
        let mut command = server.command(false);
        command.args([
            "pr",
            "view",
            "42",
            "--comments",
            "--json",
            "comments,reviews",
        ]);
        if compact {
            command.arg("--compact");
        }
        let output = command.output().unwrap();
        success(&output);
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["comments"].as_array().unwrap().len(), 2);
        assert_eq!(value["comments"][1]["body"], "Later discussion");
        assert_eq!(value["comments"][1]["created_at"], "today");
        let inline = &value["reviews"][0]["comments"][1];
        assert_eq!(inline["body"], "Later inline");
        assert_eq!(inline["diff_hunk"], "@@ -1 +2 @@");
        assert_eq!(inline["path"], "src/lib.rs");
        assert_eq!(inline["line"], 2);
        assert_eq!(inline["old_line_num"], 1);
        assert_eq!(inline["new_line_num"], 2);
        assert_eq!(inline["start_line"], 1);
        assert_eq!(inline["original_position"], 7);
        assert_eq!(inline["side"], "RIGHT");
        assert_eq!(inline["updated_at"], "today");
        assert_eq!(inline["html_url"], "inline-url");
        assert_eq!(inline["user"]["login"], "reviewer");
        assert_eq!(inline["resolver"]["login"], "maintainer");
        if compact {
            assert_eq!(value["comments"][0]["user"], json!({"login":"alice"}));
            assert_eq!(value["reviews"][0]["user"], json!({"login":"reviewer"}));
            assert!(value["reviews"][0].get("provider").is_none());
            assert!(inline.get("provider").is_none());
            assert_eq!(inline["user"], json!({"login":"reviewer"}));
            assert_eq!(inline["resolver"], json!({"login":"maintainer"}));
        } else {
            assert_eq!(value["reviews"][0]["provider"], "opaque");
            assert_eq!(inline["provider"], "opaque");
            assert_eq!(inline["user"]["avatar_url"], "opaque");
        }
        let requests = server.finish();
        assert!(requests[2].contains("issues/42/comments?page=2&limit=50"));
        assert!(requests[7].contains("reviews/3/comments?page=2&limit=50"));
    }
}

#[test]
fn compact_conversation_requires_comments_and_json_before_requests() {
    let server = Server::new(vec![]);
    for args in [
        vec!["pr", "view", "42", "--compact", "--json"],
        vec!["pr", "view", "42", "--compact", "--comments"],
    ] {
        let output = server.command(false).args(args).output().unwrap();
        failure(&output, "required arguments");
    }
    assert!(server.finish().is_empty());
}
