use super::*;

#[test]
fn governance_docs_include_site_topics_online_offline_and_in_checkouts() {
    let temp = TempDir::new().unwrap();
    let mut files: Vec<(String, String)> = [
        ("README.md", "# Governance\n".to_owned()),
        (
            "sites/docs/src/index.md",
            "# Governance guides\n".to_owned(),
        ),
        (
            "sites/docs/src/features.md",
            "# Features\nEnable project features through governance.\n".to_owned(),
        ),
        (
            "sites/docs/src/grouper.md",
            "# Grouper\nSynchronize Google Groups.\n".to_owned(),
        ),
    ]
    .into_iter()
    .map(|(path, body)| (path.to_owned(), body))
    .collect();
    files.extend(governance_data(
        tech("https://git.cmu.dev"),
        json!([{"team":{"slug":"collie","name":"Collie","repos":[{"name":"collie-api"}]}}]),
    ));
    let server = Server::with_responses(|origin| {
        let files: Vec<_> = files
            .iter()
            .map(|(path, body)| (path.as_str(), body.replace("https://git.cmu.dev", origin)))
            .collect();
        let mut tree: Vec<_> = files
            .iter()
            .map(|(path, body)| tree_entry(path, body))
            .collect();
        tree.push(tree_entry("unrelated.md", "not a governance guide"));
        tree.push(tree_entry("sites/docs/src/private.txt", "not Markdown"));
        let mut responses = source_head();
        responses.push(Response::json(
            json!({"page":1,"total_count":tree.len(),"truncated":false,"tree":tree}),
        ));
        responses.extend(files.iter().map(|(_, body)| Response {
            status: 200,
            body: body.clone(),
            headers: String::new(),
        }));
        responses
    });
    let mut cmd = command();
    cmd.args(["--host", &server.origin, "--cache-dir"])
        .arg(temp.path().join("cache"));
    cmd.args([
        "docs",
        "list",
        "--service",
        "governance",
        "--json",
        "topic,path",
    ]);
    let output = cmd.output().unwrap();
    success(&output);
    let topics: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        topics
            .iter()
            .map(|v| v["topic"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["README", "features", "grouper", "index"]
    );
    for (action, selector, field, expected) in [
        ("view", "features", "body", "Enable project features"),
        (
            "search",
            "Google Groups",
            "snippet",
            "Synchronize Google Groups",
        ),
    ] {
        let output = command()
            .args(["--host", &server.origin, "--cache-dir"])
            .arg(temp.path().join("cache"))
            .args([
                "docs",
                action,
                selector,
                "--service",
                "governance",
                "--offline",
                "--json",
                field,
            ])
            .output()
            .unwrap();
        success(&output);
        assert!(String::from_utf8_lossy(&output.stdout).contains(expected));
    }
    assert_eq!(server.finish().len(), files.len() + 3);
    for (path, body) in &files {
        let path = temp.path().join("checkout").join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    }
    let output = command()
        .args(["docs", "list", "--service", "governance", "--checkout"])
        .arg(temp.path().join("checkout"))
        .args(["--json", "topic,path"])
        .output()
        .unwrap();
    success(&output);
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        json!(topics)
    );
}

#[test]
fn docs_defaults_to_scottylabs_organization_guides() {
    let temp = TempDir::new().unwrap();
    let docs = temp.path().join("org-docs/scottylabs");
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(docs.join("index.md"), "# ScottyLabs Guides\nStart here.\n").unwrap();
    let output = command()
        .args(["docs", "list", "--checkout"])
        .arg(temp.path())
        .args(["--json", "topic,path"])
        .output()
        .unwrap();
    success(&output);
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value[0]["topic"], "index");
    assert_eq!(value[0]["path"], "org-docs/scottylabs/index.md");
}

#[test]
fn docs_lists_and_reads_topics_from_a_local_service_checkout() {
    let temp = TempDir::new().unwrap();
    let docs = temp.path().join("sites/docs/src/guides");
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(temp.path().join("README.md"), "# Kennel\nRoot guide.\n").unwrap();
    std::fs::write(
        temp.path().join("sites/docs/src/SUMMARY.md"),
        "# Generated\n",
    )
    .unwrap();
    std::fs::write(
        docs.join("features.md"),
        "# Features\nPreviews use a deployment workflow.\n",
    )
    .unwrap();

    let output = command()
        .args(["docs", "list", "--service", "kennel", "--checkout"])
        .arg(temp.path())
        .args(["--json", "topic,path,dataSource"])
        .output()
        .unwrap();
    success(&output);
    let values: Value = serde_json::from_slice(&output.stdout).unwrap();
    let values = values.as_array().unwrap();
    assert_eq!(values.len(), 2);
    assert!(
        values
            .iter()
            .any(|value| value["topic"] == "guides/features")
    );
    assert!(values.iter().any(|value| value["topic"] == "README"));
    assert!(
        values
            .iter()
            .all(|value| value["dataSource"]["kind"] == "checkout")
    );

    let output = command()
        .args([
            "docs",
            "view",
            "guides/features",
            "--service",
            "kennel",
            "--checkout",
        ])
        .arg(temp.path())
        .args(["--json", "topic,path,body"])
        .output()
        .unwrap();
    success(&output);
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["path"], "sites/docs/src/guides/features.md");
    assert!(
        value["body"]
            .as_str()
            .unwrap()
            .contains("Previews use a deployment workflow")
    );
}

#[test]
fn docs_search_is_case_insensitive_literal_and_rejects_unsafe_topics() {
    let temp = TempDir::new().unwrap();
    let docs = temp.path().join("docs/src/concepts");
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(
        docs.join("secrets.md"),
        "# Secrets\nA Vault value is read by collar.\n",
    )
    .unwrap();

    let output = command()
        .args([
            "docs",
            "search",
            "VAULT value",
            "--service",
            "infrastructure",
            "--checkout",
        ])
        .arg(temp.path())
        .args(["--json", "topic,path,snippet"])
        .output()
        .unwrap();
    success(&output);
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value[0]["topic"], "concepts/secrets");
    assert!(
        value[0]["snippet"]
            .as_str()
            .unwrap()
            .contains("Vault value")
    );

    let output = command()
        .args([
            "docs",
            "view",
            "../secrets",
            "--service",
            "infrastructure",
            "--checkout",
        ])
        .arg(temp.path())
        .output()
        .unwrap();
    failure(&output, "safe relative Markdown topic");
}
