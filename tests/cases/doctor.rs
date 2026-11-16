use super::*;

#[cfg(unix)]
#[test]
fn doctor_skips_optional_tools_and_auth_without_project_configuration() {
    let temp = TempDir::new().unwrap();
    let (project, bin) = doctor_fixture(temp.path());
    std::fs::remove_file(project.join("secretspec.toml")).unwrap();
    for tool in ["pitchfork", "bao", "secretspec"] {
        std::fs::remove_file(bin.join(tool)).unwrap();
    }
    assert!(
        git_command()
            .args(["add", "."])
            .current_dir(&project)
            .status()
            .unwrap()
            .success()
    );
    assert!(
        git_command()
            .args(["commit", "--quiet", "-m", "remove optional configuration"])
            .current_dir(&project)
            .status()
            .unwrap()
            .success()
    );
    let output = doctor_command(temp.path(), &project, &bin)
        .arg("--offline")
        .output()
        .unwrap();
    success(&output);
    let server = Server::new(vec![
        Response::json(json!({"version": "16.0.5"})),
        Response::json(json!({"login": "member"})),
    ]);
    let data_path = temp.path().join("governance/data/tech.yaml");
    let data = std::fs::read_to_string(&data_path)
        .unwrap()
        .replace("https://git.cmu.dev", &server.origin);
    std::fs::write(data_path, data).unwrap();
    let output = doctor_command(temp.path(), &project, &bin)
        .env("COLLAR_HOST", &server.origin)
        .env("COLLAR_TOKEN", "test-token")
        .output()
        .unwrap();
    let checks: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    for name in [
        "tool:pitchfork",
        "tool:bao",
        "tool:secretspec",
        "secretspec",
        "bao-auth",
    ] {
        assert!(
            checks
                .iter()
                .any(|check| check["name"] == name && check["status"] == "skipped"),
            "{checks:?}"
        );
    }
    assert_eq!(server.finish().len(), 2);
    let commands = std::fs::read_to_string(temp.path().join("commands.log")).unwrap();
    assert!(!commands.contains("bao "));
    assert!(!commands.contains("secretspec "));
}

#[cfg(unix)]
#[test]
fn doctor_requires_pitchfork_when_process_configuration_exists() {
    let temp = TempDir::new().unwrap();
    let (project, bin) = doctor_fixture(temp.path());
    std::fs::remove_file(bin.join("pitchfork")).unwrap();
    for path in ["pitchfork.toml", ".config/pitchfork.toml"] {
        let path = project.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "[daemons.web]\nrun = 'web'\n").unwrap();
        let output = doctor_command(temp.path(), &project, &bin)
            .arg("--offline")
            .output()
            .unwrap();
        failure(&output, "diagnostics failed");
        let checks: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
        assert!(
            checks
                .iter()
                .any(|check| check["name"] == "tool:pitchfork" && check["status"] == "fail")
        );
        std::fs::remove_file(path).unwrap();
    }
}

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
    std::fs::write(temp.path().join("governance/data/tech.yaml"), "broken").unwrap();
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
    let data_path = temp.path().join("governance/data/tech.yaml");
    let data = std::fs::read_to_string(&data_path)
        .unwrap()
        .replace("https://git.cmu.dev", &server.origin);
    std::fs::write(data_path, data).unwrap();
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

#[cfg(unix)]
#[test]
fn doctor_checks_effective_cache_trust_and_native_nix_reachability() {
    use std::os::unix::fs::PermissionsExt;
    const CACHE: &str = "https://scottylabs.cachix.org";
    const KEY: &str = "scottylabs.cachix.org-1:hajjEX5SLi/Y7yYloiXTt2IOr3towcTGRhMh1vu6Tjg=";
    let temp = TempDir::new().unwrap();
    let (project, bin) = doctor_fixture(temp.path());
    std::fs::write(
        bin.join("nix"),
        r#"#!/bin/sh
printf '%s\n' "$*" >> "$COLLAR_TEST_LOG"
case "$*" in
  'config show --json') printf '%s\n' "$COLLAR_TEST_NIX_CONFIG" ;;
  'store info --json') printf '%s\n' '{"trusted":true}' ;;
  'store info --store https://scottylabs.cachix.org') exit "$COLLAR_TEST_CACHE_EXIT" ;;
  '--version') exit 0 ;;
  *) exit 9 ;;
esac
"#,
    )
    .unwrap();
    std::fs::set_permissions(bin.join("nix"), std::fs::Permissions::from_mode(0o755)).unwrap();
    let config = json!({"substituters":{"value":[CACHE]},"trusted-public-keys":{"value":[KEY]},"accept-flake-config":{"value":false}});
    for (configuration, exit, expected, detail) in [
        (
            config.clone(),
            "0",
            "pass",
            "individual build availability is not checked",
        ),
        (config.clone(), "1", "fail", "could not reach"),
        (
            {
                let mut c = config.clone();
                c["trusted-public-keys"]["value"] = json!([]);
                c
            },
            "0",
            "fail",
            "does not trust",
        ),
        (
            {
                let mut c = config.clone();
                c["substituters"]["value"] = json!([]);
                c
            },
            "0",
            "fail",
            "not an effective",
        ),
    ] {
        let server = Server::new(vec![
            Response::json(json!({"version":"17.0.1"})),
            Response::json(json!({"login":"member"})),
        ]);
        let output = doctor_command(temp.path(), &project, &bin)
            .env("COLLAR_HOST", &server.origin)
            .env("COLLAR_TOKEN", "test-token")
            .env("XDG_DATA_HOME", temp.path().join("data"))
            .env("COLLAR_TEST_NIX_CONFIG", configuration.to_string())
            .env("COLLAR_TEST_CACHE_EXIT", exit)
            .output()
            .unwrap();
        let checks: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
        let cache = checks.iter().find(|c| c["name"] == "nix-cache").unwrap();
        assert_eq!(cache["status"], expected, "{checks:?}");
        assert!(
            cache["detail"].as_str().unwrap().contains(detail),
            "{cache}"
        );
        server.finish();
    }
}
