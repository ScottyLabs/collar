use super::*;
use std::os::unix::fs::PermissionsExt;

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

#[test]
fn governance_validator_uses_canonical_command_and_preserves_tool_exit_code() {
    let temp = TempDir::new().unwrap();
    let checkout = temp.path().join("governance");
    std::fs::create_dir_all(checkout.join("crates/governance-cli")).unwrap();
    std::fs::create_dir_all(checkout.join("data")).unwrap();
    for file in [
        checkout.join("Cargo.toml"),
        checkout.join("crates/governance-cli/Cargo.toml"),
        checkout.join("data/org.toml"),
    ] {
        std::fs::write(file, "fixture\n").unwrap();
    }
    let bin = temp.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let cargo = bin.join("cargo");
    std::fs::write(
        &cargo,
        "#!/bin/sh\nprintf 'cwd:%s\\n' \"$PWD\" > \"$COLLAR_CARGO_LOG\"\nfor value in \"$@\"; do printf 'arg:%s\\n' \"$value\" >> \"$COLLAR_CARGO_LOG\"; done\nexit 7\n",
    )
    .unwrap();
    std::fs::set_permissions(cargo, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::join_paths([bin.clone()]).unwrap();
    let output = command()
        .env("PATH", path)
        .env("COLLAR_CARGO_LOG", temp.path().join("cargo.log"))
        .args(["governance", "validate", "--checkout"])
        .arg(&checkout)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(7));
    assert!(String::from_utf8_lossy(&output.stderr).contains("cargo exited with exit status: 7"));
    let log = std::fs::read_to_string(temp.path().join("cargo.log")).unwrap();
    assert!(log.contains(&format!("cwd:{}\n", checkout.display())));
    assert!(log.contains("arg:run\narg:-p\narg:governance\narg:--\narg:validate\n"));
}

#[test]
fn governance_run_delegates_literal_arguments_through_devenv_and_propagates_failure() {
    let temp = TempDir::new().unwrap();
    let checkout = temp.path().join("governance");
    std::fs::create_dir_all(checkout.join("crates/governance-cli")).unwrap();
    std::fs::create_dir_all(checkout.join("data")).unwrap();
    for file in [
        checkout.join("Cargo.toml"),
        checkout.join("crates/governance-cli/Cargo.toml"),
        checkout.join("data/org.toml"),
    ] {
        std::fs::write(file, "fixture\n").unwrap();
    }
    let bin = temp.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let devenv = bin.join("devenv");
    std::fs::write(
        &devenv,
        "#!/bin/sh\nprintf 'cwd:%s\\n' \"$PWD\" > \"$COLLAR_DEVENV_LOG\"\nfor value in \"$@\"; do printf 'arg:%s\\n' \"$value\" >> \"$COLLAR_DEVENV_LOG\"; done\nexit 9\n",
    )
    .unwrap();
    std::fs::set_permissions(devenv, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::join_paths([bin]).unwrap();
    let output = command()
        .env("PATH", path)
        .env("COLLAR_DEVENV_LOG", temp.path().join("devenv.log"))
        .args(["governance", "run", "--checkout"])
        .arg(&checkout)
        .args([
            "--",
            "schema",
            "--output-dir",
            "-schema output",
            "$(literal argument)",
        ])
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(9),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("devenv exited with exit status: 9"));
    let log = std::fs::read_to_string(temp.path().join("devenv.log")).unwrap();
    assert!(log.contains(&format!("cwd:{}\n", checkout.display())));
    assert!(log.contains("arg:shell\narg:--\narg:cargo\narg:run\narg:-p\narg:governance\narg:--\narg:schema\narg:--output-dir\narg:-schema output\narg:$(literal argument)\n"));
}
