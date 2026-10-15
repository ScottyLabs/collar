use super::*;

#[test]
fn changelog_uses_latest_pinned_snapshots_and_explicit_offline_cache() {
    let temp = TempDir::new().unwrap();
    let files = [
        ("governance.md", "# Governance\n"),
        ("infrastructure.md", "# Infrastructure\n"),
        ("kennel.md", "# Kennel\n"),
        (
            "updates/017-current.md",
            "# Current\n\nNo action required.\n",
        ),
    ];
    let mut responses = source_head();
    responses.push(Response::json(
        json!({"page":1,"total_count":4,"truncated":false,
        "tree":files.iter().map(|(path, body)| tree_entry(path, body)).collect::<Vec<_>>()}),
    ));
    responses.extend(files.iter().map(|(_, body)| Response {
        status: 200,
        body: body.to_string(),
        headers: String::new(),
    }));
    let server = Server::new(responses);
    let mut cmd = command();
    cmd.args(["--host", &server.origin, "--cache-dir"])
        .arg(temp.path())
        .args(["changelog", "update", "17", "--json"]);
    let output = cmd.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["title"], "Current");
    assert_eq!(value["dataSource"]["repository"], "ScottyLabs/changelog");
    assert_eq!(value["dataSource"]["revision"], SOURCE_REVISION);
    let requests = server.finish();
    assert!(requests[0].contains("/repos/ScottyLabs/changelog "));
    assert!(
        requests[3..]
            .iter()
            .all(|request| request.contains(SOURCE_REVISION))
    );
    let output = cmd.arg("--offline").output().unwrap();
    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["dataSource"]["offline"], true);
}

#[test]
fn changelog_reads_numbered_announcements_and_rejects_ambiguity() {
    let temp = TempDir::new().unwrap();
    std::fs::create_dir(temp.path().join("updates")).unwrap();
    for (path, body) in [
        ("updates/001-first.md", "# First\n\nNo action required.\n"),
        (
            "updates/017-current.md",
            "# Current\n\nAction required if using previews.\n",
        ),
        ("updates/README.md", "not an announcement"),
        ("kennel.md", "# Kennel\n\n## 2026-10-14\n\nChanges\n"),
    ] {
        std::fs::write(temp.path().join(path), body).unwrap();
    }
    let output = command()
        .args(["changelog", "updates", "--checkout"])
        .arg(temp.path())
        .args(["--json", "number,title"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        json!([
            {"number":17,"title":"Current"},{"number":1,"title":"First"}
        ])
    );
    let output = command()
        .args(["changelog", "update", "17", "--checkout"])
        .arg(temp.path())
        .args(["--json", "body"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("Action required"));
    let output = command()
        .args(["changelog", "view", "kennel", "--checkout"])
        .arg(temp.path())
        .args(["--json", "title"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        json!({"title":"Kennel"})
    );
    std::fs::write(temp.path().join("updates/017-duplicate.md"), "# Duplicate").unwrap();
    let output = command()
        .args(["changelog", "update", "17", "--checkout"])
        .arg(temp.path())
        .output()
        .unwrap();
    failure(&output, "duplicate announcement number");
}
