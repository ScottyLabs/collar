use super::*;

#[cfg(unix)]
#[test]
fn offline_doctor_reports_manual_checks_and_never_resolves_secrets() {
    let temp = TempDir::new().unwrap();
    let (project, bin) = doctor_fixture(temp.path());
    let output = doctor_command(temp.path(), &project, &bin)
        .arg("--offline")
        .output()
        .unwrap();
    success(&output);
    let checks: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert!(checks.iter().all(|check| check["status"] != "fail"));
    for name in ["forge", "forge-auth", "bao-auth"] {
        assert!(
            checks
                .iter()
                .any(|check| check["name"] == name && check["status"] == "skipped")
        );
    }
    assert!(
        checks
            .iter()
            .any(|check| check["name"] == "linked-accounts" && check["status"] == "manual")
    );
    let commands = std::fs::read_to_string(temp.path().join("commands.log")).unwrap();
    assert!(!commands.contains("token lookup"));
    assert!(!commands.contains("secretspec check"));
    assert!(!String::from_utf8_lossy(&output.stdout).contains("never-print-tool-secret"));
    std::fs::write(
        project.join("secretspec.toml"),
        "[project]\nname = 'collie-api'\nrevision = '1.0'\n[profiles.dev]\n",
    )
    .unwrap();
    std::fs::write(
        temp.path().join("governance/data/teams/extra.toml"),
        "broken = [",
    )
    .unwrap();
    let output = doctor_command(temp.path(), &project, &bin)
        .arg("--offline")
        .output()
        .unwrap();
    failure(&output, "diagnostics failed");
    let checks: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        checks
            .iter()
            .any(|check| check["name"] == "governance" && check["status"] == "fail")
    );
    assert!(
        checks
            .iter()
            .any(|check| check["name"] == "governance-revision" && check["status"] == "fail")
    );
    assert!(
        checks
            .iter()
            .any(|check| check["name"] == "secretspec" && check["status"] == "skipped")
    );
}

#[cfg(unix)]
#[test]
fn online_doctor_verifies_forge_and_bao_without_echoing_tool_output() {
    let temp = TempDir::new().unwrap();
    let (project, bin) = doctor_fixture(temp.path());
    let server = Server::new(vec![
        Response::json(json!({"version": "16.0.5"})),
        Response::json(json!({"login": "member"})),
    ]);
    let org_path = temp.path().join("governance/data/org.toml");
    let org = std::fs::read_to_string(&org_path)
        .unwrap()
        .replace("https://git.cmu.dev", &server.origin);
    std::fs::write(org_path, org).unwrap();
    let output = doctor_command(temp.path(), &project, &bin)
        .env("COLLAR_HOST", &server.origin)
        .env("COLLAR_TOKEN", "test-token")
        .output()
        .unwrap();
    failure(&output, "diagnostics failed");
    let checks: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    for name in ["forge", "forge-auth", "bao-auth"] {
        assert!(
            checks
                .iter()
                .any(|check| check["name"] == name && check["status"] == "pass")
        );
    }
    assert!(
        checks
            .iter()
            .any(|check| check["name"] == "governance-revision" && check["status"] == "fail")
    );
    let requests = server.finish();
    assert!(requests[0].starts_with("GET /api/v1/version "));
    assert!(requests[1].starts_with("GET /api/v1/user "));
    let commands = std::fs::read_to_string(temp.path().join("commands.log")).unwrap();
    assert!(commands.contains("bao token lookup -format=json"));
    assert!(!commands.contains("secretspec check"));
    assert!(!String::from_utf8_lossy(&output.stdout).contains("never-print-tool-secret"));
}

#[cfg(unix)]
#[test]
fn doctor_aggregates_tool_namespace_and_profile_failures() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempDir::new().unwrap();
    let (project, bin) = doctor_fixture(temp.path());
    std::fs::remove_file(bin.join("nix")).unwrap();
    std::fs::write(
        bin.join("bao"),
        "#!/bin/sh\nprintf 'never-print-tool-secret' >&2\nexit 7\n",
    )
    .unwrap();
    std::fs::set_permissions(bin.join("bao"), std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::write(
        project.join("secretspec.toml"),
        "[project]\nname = 'collie-api'\nrevision = '1.0'\n[profiles.dev]\n",
    )
    .unwrap();
    let output = doctor_command(temp.path(), &project, &bin)
        .arg("--offline")
        .output()
        .unwrap();
    failure(&output, "diagnostics failed");
    let checks: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    for name in ["tool:nix", "tool:bao", "secretspec"] {
        assert!(
            checks
                .iter()
                .any(|check| check["name"] == name && check["status"] == "fail")
        );
    }
    assert!(!String::from_utf8_lossy(&output.stdout).contains("never-print-tool-secret"));
    std::fs::write(
        project.join("secretspec.toml"),
        "[project]\nname = 'collie'\nrevision = '1.0'\n[profiles.dev]\n",
    )
    .unwrap();
    let output = doctor_command(temp.path(), &project, &bin)
        .args(["--offline", "--profile", "prod"])
        .output()
        .unwrap();
    failure(&output, "diagnostics failed");
    let checks: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert!(checks.iter().any(|check| check["name"] == "secretspec"
        && check["detail"].as_str().unwrap().contains("profile prod")));
}
