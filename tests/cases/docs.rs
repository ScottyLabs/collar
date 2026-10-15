use super::*;

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
