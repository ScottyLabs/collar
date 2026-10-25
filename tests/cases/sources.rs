use super::*;

#[test]
fn canonical_governance_snapshots_paginate_pin_files_and_require_explicit_offline() {
    let temp = TempDir::new().unwrap();
    let server = Server::with_responses(|origin| {
        let org = format!(
            "[org.forgejo]\norg = 'ScottyLabs'\nurl = '{origin}'\n[org.keycloak]\nurl = 'https://idp.scottylabs.org'\nrealm = 'scottylabs'\n"
        );
        let team =
            "[team]\nslug = 'collie'\nname = 'Collie'\n[[team.repos]]\nname = 'collie-api'\n";
        let mut responses = source_head();
        responses.extend([
            Response::json(json!({"page": 1, "total_count": 2, "truncated": true, "tree": [tree_entry("data/org.toml", &org)]})),
            Response::json(json!({"page": 2, "total_count": 2, "truncated": false, "tree": [tree_entry("data/teams/collie.toml", team)]})),
            Response {status: 200, body: org, headers: String::new()},
            Response {status: 200, body: team.into(), headers: String::new()},
        ]);
        responses.extend(source_head());
        responses.push(Response {
            status: 503,
            body: "never-print-server-secret".into(),
            headers: String::new(),
        });
        responses
    });
    let run = |offline| {
        let mut cmd = command();
        cmd.args(["--host", &server.origin, "--cache-dir"])
            .arg(temp.path())
            .args([
                "governance",
                "project",
                "view",
                "collie-api",
                "--json",
                "slug,secretNamespace,source,dataSource",
            ]);
        if offline {
            cmd.arg("--offline");
        }
        cmd.output().unwrap()
    };
    let output = run(false);
    success(&output);
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["slug"], "collie");
    assert_eq!(value["secretNamespace"], "collie");
    assert_eq!(value["dataSource"]["revision"], SOURCE_REVISION);
    assert_eq!(value["dataSource"]["offline"], false);
    success(&run(true));
    let source_file = std::path::Path::new(value["source"].as_str().unwrap());
    // The fixture is stored at REVISION/data/teams/collie.toml.
    let cache = source_file.ancestors().nth(4).unwrap();
    let obsolete = cache.join("2".repeat(40));
    std::fs::create_dir(&obsolete).unwrap();
    std::fs::write(obsolete.join("old-content"), "obsolete").unwrap();
    success(&run(false));
    assert!(!obsolete.exists());
    assert!(source_file.is_file());
    std::fs::create_dir(&obsolete).unwrap();
    let foreign = command()
        .args(["--host", "other.example", "--cache-dir"])
        .arg(temp.path())
        .args(["governance", "project", "list", "--offline", "--json"])
        .output()
        .unwrap();
    failure(&foreign, "no readable cached governance snapshot");
    let project = command()
        .args(["--host", &server.origin, "--cache-dir"])
        .arg(temp.path())
        .args([
            "-R",
            "ScottyLabs/collie-api",
            "project",
            "view",
            "--offline",
            "--json",
            "slug,dataSource",
        ])
        .output()
        .unwrap();
    success(&project);

    let failed = run(false);
    failure(&failed, "HTTP 503");
    assert!(obsolete.is_dir(), "failed refresh must not run eviction");
    assert!(failed.stdout.is_empty());
    let cached = run(true);
    success(&cached);
    let value: Value = serde_json::from_slice(&cached.stdout).unwrap();
    assert_eq!(value["dataSource"]["offline"], true);
    let requests = server.finish();
    assert_eq!(requests.len(), 9);
    assert!(requests[2].contains("recursive=true&page=1&per_page=1000"));
    assert!(requests[3].contains("page=2"));
    for request in &requests[4..6] {
        assert!(request.contains(&format!("ref={SOURCE_REVISION}")));
    }
    let mut source = command();
    source
        .args([
            "--host",
            value["dataSource"]["host"].as_str().unwrap(),
            "--cache-dir",
        ])
        .arg(temp.path())
        .args([
            "governance",
            "project",
            "view",
            "collie-api",
            "--offline",
            "--json",
            "source",
        ]);
    let output = source.output().unwrap();
    success(&output);
    let path: Value = serde_json::from_slice(&output.stdout).unwrap();
    let file = std::path::Path::new(path["source"].as_str().unwrap());
    let content = std::fs::read_to_string(file).unwrap();
    std::fs::write(file, content.replace("Collie", "Tamper")).unwrap();
    failure(
        &source.output().unwrap(),
        "does not match its pinned Git blob",
    );
    std::fs::remove_file(path["source"].as_str().unwrap()).unwrap();
    failure(&source.output().unwrap(), "cached source is incomplete");
}

#[test]
fn source_snapshots_reject_unsafe_incomplete_and_symlink_trees_before_raw_reads() {
    for (entry, total, message) in [
        (
            json!({"path":"data/teams/../../escape", "type":"blob", "mode":"100644", "size":2, "sha":SOURCE_REVISION}),
            1,
            "unsafe path",
        ),
        (
            json!({"path":"data/org.toml", "type":"blob", "mode":"120000", "size":2, "sha":SOURCE_REVISION}),
            1,
            "symlinks",
        ),
        (
            json!({"path":"data/org.toml", "type":"blob", "mode":"100644", "size":2, "sha":SOURCE_REVISION}),
            2,
            "incomplete",
        ),
        (
            json!({"path":"data/org.toml", "type":"blob", "mode":"100644", "size":u64::MAX, "sha":SOURCE_REVISION}),
            1,
            "exceeds 8 MiB",
        ),
    ] {
        let temp = TempDir::new().unwrap();
        let mut responses = source_head();
        responses.push(Response::json(
            json!({"page":1,"total_count":total,"truncated":false,"tree":[entry]}),
        ));
        let server = Server::new(responses);
        let output = command()
            .args(["--host", &server.origin, "--cache-dir"])
            .arg(temp.path())
            .args(["governance", "project", "list", "--json"])
            .output()
            .unwrap();
        failure(&output, message);
        assert!(output.stdout.is_empty());
        assert_eq!(server.finish().len(), 3);
    }
}

#[test]
fn failed_snapshot_download_does_not_publish_an_offline_source() {
    let temp = TempDir::new().unwrap();
    let mut responses = source_head();
    responses.extend([
        Response::json(json!({"page":1,"total_count":1,"truncated":false,"tree":[tree_entry("data/org.toml", "{}")]})),
        Response {status:403,body:"never-print-server-secret".into(),headers:String::new()},
    ]);
    let server = Server::new(responses);
    let mut cmd = command();
    cmd.args(["--host", &server.origin, "--cache-dir"])
        .arg(temp.path())
        .args(["governance", "project", "list", "--json"]);
    let output = cmd.output().unwrap();
    failure(&output, "HTTP 403");
    assert!(!String::from_utf8_lossy(&output.stderr).contains("never-print-server-secret"));
    let output = cmd.arg("--offline").output().unwrap();
    failure(&output, "no readable cached governance snapshot");
    assert_eq!(server.finish().len(), 4);
}
