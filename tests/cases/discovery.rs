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
    assert_eq!(project["leads"], json!(["lead", "other"]));
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
fn governance_accepts_manually_managed_project_google_groups() {
    let checkout = TempDir::new().unwrap();
    write_governance(
        checkout.path(),
        tech("https://git.cmu.dev"),
        json!([{"team": {
            "slug": "courses", "name": "Courses",
            "projects": [{
                "slug": "courses-web", "name": "Courses web",
                "repos": [{"name": "courses-web"}],
                "google_groups": [{"email": "sl-courses@andrew.cmu.edu", "kind": "contact", "manual": true}]
            }]
        }}]),
    );
    let output = command()
        .args(["governance", "project", "list", "--checkout"])
        .arg(checkout.path())
        .args(["--json", "slug,name,team,repos"])
        .output()
        .unwrap();
    success(&output);
    let projects: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(projects[0]["slug"], "courses-web");
    assert_eq!(projects[0]["team"], "courses");
    assert_eq!(projects[0]["repos"][0]["name"], "courses-web");
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
            "slug": "collie", "team": "tech", "leads": ["lead"], "members": ["member"],
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

#[test]
fn governance_team_list_preserves_projects_and_empty_groups() {
    let checkout = TempDir::new().unwrap();
    write_governance(
        checkout.path(),
        tech("https://git.cmu.dev"),
        json!([
            {"team": {"slug":"devops","name":"DevOps","description":"Infrastructure and deployment","leads":["Lead"],"members":["Member"],"repos":[{"name":"shared-tools"}],"projects":[
                {"slug":"infrastructure","name":"Infrastructure","members":["ProjectMember"],"repos":[{"name":"infrastructure","features":{"kennel":{}}},{"name":"infrastructure-vendor"}]},
                {"slug":"planning","name":"Planning","description":"Work before a repository exists"}
            ]}},
            {"team":{"slug":"empty","name":"Empty team"}}
        ]),
    );
    let output = command()
        .args(["governance", "team", "list", "--checkout"])
        .arg(checkout.path())
        .args(["--json"])
        .output()
        .unwrap();
    success(&output);
    let teams: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(teams.as_array().unwrap().len(), 2);
    assert_eq!(teams[0]["slug"], "devops");
    assert_eq!(teams[0]["name"], "DevOps");
    assert_eq!(teams[0]["description"], "Infrastructure and deployment");
    assert_eq!(teams[0]["leads"], json!(["lead"]));
    assert_eq!(teams[0]["repos"][0]["name"], "shared-tools");
    assert_eq!(teams[0]["projects"][0]["slug"], "infrastructure");
    assert_eq!(
        teams[0]["projects"][0]["repos"].as_array().unwrap().len(),
        2
    );
    assert_eq!(
        teams[0]["projects"][0]["repos"][0]["features"],
        json!({"kennel":{}})
    );
    assert_eq!(
        teams[0]["projects"][0]["members"],
        json!(["member", "projectmember"])
    );
    assert_eq!(teams[0]["projects"][1]["repos"], json!([]));
    assert_eq!(teams[0]["dataSource"]["kind"], "checkout");
    assert_eq!(teams[1]["projects"], json!([]));
    assert_eq!(teams[1]["repos"], json!([]));
    let output = command()
        .args(["governance", "team", "list", "--checkout"])
        .arg(checkout.path())
        .args(["--json", "slug,projects"])
        .output()
        .unwrap();
    success(&output);
    let selected: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(selected[0].as_object().unwrap().len(), 2);
    assert!(selected[0].get("dataSource").is_none());
}
