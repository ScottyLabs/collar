use super::*;

#[test]
fn governance_uses_project_namespace_and_team_leads() {
    let checkout = TempDir::new().unwrap();
    let team = json!({"team": {
        "slug": "tech", "name": "Tech", "leads": ["Lead", "Other"],
        "projects": [{"slug": "collie", "name": "Collie", "repos": [
            {"name": "collie-api", "features": {"kennel": {}}}
        ]}]
    }});
    write_governance(checkout.path(), tech("https://git.cmu.dev"), json!([team]));
    let output = command()
        .args(["governance", "project", "view", "collie-api", "--checkout"])
        .arg(checkout.path())
        .args(["--json", "slug,secretNamespace,leads,repos"])
        .output()
        .unwrap();
    success(&output);
    let project: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(project["secretNamespace"], "collie");
    assert_eq!(project["leads"], json!(["Lead", "Other"]));
    assert_eq!(project["repos"][0]["features"], json!({"kennel": {}}));
    write_governance(
        checkout.path(),
        tech("https://git.cmu.dev"),
        json!([{"team": {"name": "Missing slug"}}]),
    );
    let output = command()
        .args(["governance", "project", "list", "--checkout"])
        .arg(checkout.path())
        .output()
        .unwrap();
    failure(&output, "invalid governance data");
}

#[test]
fn project_context_uses_canonical_ownership_and_repository_specific_features() {
    let temp = TempDir::new().unwrap();
    project_governance(temp.path());
    let output = command()
        .args([
            "-R",
            "scottylabs/COLLIE-api",
            "project",
            "view",
            "--checkout",
        ])
        .arg(temp.path())
        .args([
            "--json",
            "slug,team,leads,members,features,secretNamespace,declaredOnly,linkedAccountsUrl",
        ])
        .output()
        .unwrap();
    success(&output);
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        json!({
            "slug": "collie", "team": "tech", "leads": ["Lead"], "members": ["member"],
            "features": {"kennel": {}}, "secretNamespace": "collie", "declaredOnly": true,
            "linkedAccountsUrl": "https://idp.scottylabs.org/realms/scottylabs/account/account-security/linked-accounts"
        })
    );
    for (repo, expected) in [
        ("other.example/ScottyLabs/collie-api", "different forge"),
        ("someone/collie-api", "outside the governance organization"),
        ("ScottyLabs/absent", "not declared in governance"),
    ] {
        let output = command()
            .args(["-R", repo, "project", "view", "--checkout"])
            .arg(temp.path())
            .output()
            .unwrap();
        failure(&output, expected);
        assert!(output.stdout.is_empty());
    }
    let mut team = collie_team();
    team["team"]["projects"]
        .as_array_mut()
        .unwrap()
        .push(json!({"slug": "duplicate", "name": "Duplicate", "repos": [{"name": "COLLIE-API"}]}));
    write_governance(temp.path(), tech("https://git.cmu.dev"), json!([team]));
    let output = command()
        .args([
            "-R",
            "ScottyLabs/collie-api",
            "project",
            "view",
            "--checkout",
        ])
        .arg(temp.path())
        .output()
        .unwrap();
    failure(&output, "ambiguous governance ownership");
}
