use super::*;

#[test]
fn governance_uses_nested_project_namespace_and_inherited_leads() {
    let checkout = TempDir::new().unwrap();
    std::fs::create_dir_all(checkout.path().join("data/teams")).unwrap();
    let file = checkout.path().join("data/teams/team.toml");
    std::fs::write(&file, "[team]\nslug = 'tech'\nname = 'Tech'\nleads = ['Lead']\n[[team.projects]]\nslug = 'collie'\nname = 'Collie'\nleads = ['lead', 'Other']\n[[team.projects.repos]]\nname = 'collie-api'\n[team.projects.repos.features]\nkennel = {}\n").unwrap();
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
    assert_eq!(project["repos"][0]["features"]["kennel"], json!({}));
    std::fs::write(file, "[team]\nname = 'Missing slug'\n").unwrap();
    let output = command()
        .args(["governance", "project", "list", "--checkout"])
        .arg(checkout.path())
        .output()
        .unwrap();
    failure(&output, "invalid governance team");
}

#[test]
fn infrastructure_discovers_configurations_and_refuses_missing_services() {
    let checkout = TempDir::new().unwrap();
    let host = checkout.path().join("modules/hosts/deploy-01");
    std::fs::create_dir_all(&host).unwrap();
    std::fs::create_dir_all(checkout.path().join("modules/roles")).unwrap();
    std::fs::write(host.join("configuration.nix"), "{}").unwrap();
    std::fs::write(host.join("kennel.nix"), "{}").unwrap();
    std::fs::write(checkout.path().join("modules/roles/deploy-01.nix"), "{}").unwrap();
    let output = command()
        .args(["infra", "service", "kennel", "--checkout"])
        .arg(checkout.path())
        .args(["--json", "host"])
        .output()
        .unwrap();
    success(&output);
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        json!([{ "host": "deploy-01" }])
    );
    let output = command()
        .args(["infra", "service", "missing", "--checkout"])
        .arg(checkout.path())
        .output()
        .unwrap();
    failure(&output, "no host configuration");
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
    let file = temp.path().join("data/teams/team.toml");
    let mut content = std::fs::read_to_string(&file).unwrap();
    content.push_str("\n[[team.projects]]\nname = 'Duplicate'\nslug = 'duplicate'\n[[team.projects.repos]]\nname = 'COLLIE-API'\n");
    std::fs::write(file, content).unwrap();
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
