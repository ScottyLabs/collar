use super::*;

#[test]
fn service_access_resolves_selected_repository_and_declared_feature_families() {
    let temp = TempDir::new().unwrap();
    let declared = json!({
        "ai_gateway": {
            "prod_monthly_budget": 10.0, "dev_monthly_budget": 1.0,
            "prod_rpm_limit": 60, "dev_rpm_limit": 10, "prod_tpm_limit": 1000, "dev_tpm_limit": 100,
            "prod_max_parallel_requests": 4, "dev_max_parallel_requests": 1
        },
        "cdn": {},
        "docs": {},
        "kennel": {},
        "mobile_app": {"google_play": "org.scottylabs.collie", "app_store": null},
        "oidc_client": {"admin": false, "public": false, "groups": []},
        "posthog": {"server_side": false},
        "sentry": {}
    });
    let team = json!({"team": {
        "slug": "tech", "name": "Tech", "leads": ["Lead"], "members": ["member"],
        "projects": [{"slug": "collie", "name": "Collie", "repos": [
            {"name": "collie-api", "features": declared.clone()}
        ]}]
    }});
    let mut tech = tech("https://git.cmu.dev");
    tech["keycloak"]["url"] = json!("https://identity.example.test");
    tech["keycloak"]["realm"] = json!("custom-realm");
    write_governance(temp.path(), tech, json!([team]));

    let output = command()
        .args([
            "-R",
            "ScottyLabs/collie-api",
            "governance",
            "service-access",
            "--checkout",
        ])
        .arg(temp.path())
        .args([
            "--json",
            "repository,team,leads,members,features,declaredOnly,linkedAccountsUrl",
        ])
        .output()
        .unwrap();
    success(&output);
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["repository"], "ScottyLabs/collie-api");
    assert_eq!(value["team"], "tech");
    assert_eq!(value["leads"], json!(["Lead"]));
    assert_eq!(value["members"], json!(["member"]));
    assert_eq!(value["declaredOnly"], true);
    assert_eq!(
        value["linkedAccountsUrl"],
        "https://identity.example.test/realms/custom-realm/account/account-security/linked-accounts"
    );
    let reported = value["features"].as_array().unwrap();
    assert_eq!(reported.len(), 8);
    for feature in reported {
        let name = feature["name"].as_str().unwrap();
        assert_eq!(feature["declaration"], declared[name], "{name}");
    }
}

#[test]
fn service_access_rejects_wrong_forge_and_organization_for_selected_repository() {
    let temp = TempDir::new().unwrap();
    project_governance(temp.path());
    for (repo, expected) in [
        ("other.example/ScottyLabs/collie-api", "different forge"),
        ("someone/collie-api", "outside the governance organization"),
    ] {
        let output = command()
            .args(["-R", repo, "governance", "service-access", "--checkout"])
            .arg(temp.path())
            .output()
            .unwrap();
        failure(&output, expected);
        assert!(output.stdout.is_empty());
    }
}
