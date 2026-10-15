use super::*;

#[test]
fn service_access_resolves_selected_repository_and_declared_feature_families() {
    let temp = TempDir::new().unwrap();
    project_governance(temp.path());
    let team = temp.path().join("data/teams/team.toml");
    std::fs::write(
        team,
        "[team]\nslug = 'tech'\nname = 'Tech'\nleads = ['Lead']\nmembers = ['member']\n[[team.projects]]\nslug = 'collie'\nname = 'Collie'\n[[team.projects.repos]]\nname = 'collie-api'\n[team.projects.repos.features]\nai_gateway = {}\ncdn = {}\ndocs = {}\nkennel = {}\nmobile_app = {}\noidc_client = {}\nposthog = {}\nsentry = {}\n",
    )
    .unwrap();
    let org = temp.path().join("data/org.toml");
    std::fs::write(
        org,
        "[org.forgejo]\norg = 'ScottyLabs'\nurl = 'https://git.cmu.dev'\n[org.keycloak]\nurl = 'https://identity.example.test'\nrealm = 'custom-realm'\n",
    )
    .unwrap();

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
    let features = value["features"].as_array().unwrap();
    assert_eq!(features.len(), 8);
    for name in [
        "ai_gateway",
        "cdn",
        "docs",
        "kennel",
        "mobile_app",
        "oidc_client",
        "posthog",
        "sentry",
    ] {
        assert!(features.iter().any(|feature| feature["name"] == name));
    }
    assert!(
        features
            .iter()
            .all(|feature| feature["declaration"] == json!({}))
    );
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
