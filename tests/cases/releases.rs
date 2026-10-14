use super::*;

fn release() -> Value {
    json!({
        "id": 41,
        "tag_name": "v1.2.0",
        "name": "Terrier 1.2.0",
        "body": "Notes",
        "draft": true,
        "prerelease": true,
        "html_url": "https://forge.example/ScottyLabs/terrier/releases/tag/v1.2.0",
        "assets": [{"name": "terrier.tar.gz", "browser_download_url": "https://forge.example/assets/1", "size": 123}]
    })
}

fn release_command(server: &Server, args: &[&str]) -> Command {
    let mut command = command();
    command
        .args(["--host", &server.origin, "-R", "ScottyLabs/terrier"])
        .args(args)
        .env("COLLAR_HOST", &server.origin)
        .env("COLLAR_TOKEN", "fixture-token");
    command
}

fn request_body(request: &str) -> Value {
    serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap()
}

#[test]
fn release_edit_resolves_tag_then_clears_draft_and_prerelease() {
    let server = Server::new(vec![
        Response::json(release()),
        Response::json(
            json!({"id":41,"tag_name":"v1.2.0","name":"Terrier 1.2.0","body":"Notes","draft":false,"prerelease":false,"assets":[]}),
        ),
    ]);
    let output = release_command(
        &server,
        &["release", "edit", "v1.2.0", "--publish", "--stable"],
    )
    .output()
    .unwrap();
    success(&output);
    let requests = server.finish();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].starts_with("GET /api/v1/repos/ScottyLabs/terrier/releases/tags/v1.2.0 "));
    assert!(requests[1].starts_with("PATCH /api/v1/repos/ScottyLabs/terrier/releases/41 "));
    assert_eq!(
        request_body(&requests[1]),
        json!({"draft":false,"prerelease":false})
    );
}

#[test]
fn release_create_sends_literal_markdown_to_native_endpoint() {
    let server = Server::new(vec![Response::json(release())]);
    let output = release_command(
        &server,
        &[
            "release",
            "create",
            "v1.2.0",
            "--title",
            "Terrier 1.2.0",
            "--notes",
            "# Release\n\nLiteral `markdown`.\n",
            "--draft",
            "--prerelease",
        ],
    )
    .output()
    .unwrap();
    success(&output);
    let requests = server.finish();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].starts_with("POST /api/v1/repos/ScottyLabs/terrier/releases "));
    assert_eq!(
        request_body(&requests[0]),
        json!({"tag_name":"v1.2.0","name":"Terrier 1.2.0","body":"# Release\n\nLiteral `markdown`.\n","draft":true,"prerelease":true})
    );
}

#[test]
fn release_validation_and_json_field_errors_happen_before_requests() {
    let server = Server::new(vec![]);
    let output = release_command(&server, &["release", "view", "v1.2.0", "--json", "unknown"])
        .output()
        .unwrap();
    failure(&output, "unsupported JSON field: unknown");
    assert!(server.finish().is_empty());
}
