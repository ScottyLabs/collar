use super::*;

#[test]
fn saves_and_unsets_host_defaults_atomically() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("settings/config.toml");
    let output = isolated_command()
        .arg("--config")
        .arg(&path)
        .args(["config", "set", "host", "example.test"])
        .output()
        .unwrap();
    success(&output);
    let output = isolated_command()
        .arg("--config")
        .arg(&path)
        .args(["config", "get", "--json", "host"])
        .output()
        .unwrap();
    success(&output);
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        json!({"host":"https://example.test/"})
    );
    let output = isolated_command()
        .arg("--config")
        .arg(&path)
        .args(["config", "unset", "host"])
        .output()
        .unwrap();
    success(&output);
    let output = isolated_command()
        .arg("--config")
        .arg(&path)
        .args(["config", "get", "--json", "host"])
        .output()
        .unwrap();
    success(&output);
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        json!({"host":null})
    );
}

#[test]
fn rejects_corrupt_missing_and_secret_configuration_without_overwriting() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("config.toml");
    let output = isolated_command()
        .arg("--config")
        .arg(&path)
        .args(["config", "get"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    for text in [
        "token = 'very-secret-value'",
        "host = ['very-secret-value']",
        "host = '",
    ] {
        std::fs::write(&path, text).unwrap();
        let output = isolated_command()
            .arg("--config")
            .arg(&path)
            .args(["config", "set", "host", "git.cmu.dev"])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("very-secret-value"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
    }
    let output = command()
        .args(["config", "set", "host", "git.cmu.dev"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("configuration disabled"));
}

#[test]
fn saved_host_controls_api_and_token_binding_but_not_explicit_other_host() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("config.toml");
    let configured = Server::new(vec![Response::json(json!({"version": "test"}))]);
    std::fs::write(&path, format!("host = '{}'\n", configured.origin)).unwrap();
    let output = isolated_command()
        .arg("--config")
        .arg(&path)
        .env("COLLAR_TOKEN", "test-token")
        .args(["api", "version"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let requests = configured.finish();
    assert!(
        requests[0]
            .to_lowercase()
            .contains("authorization: token test-token")
    );
    let other = Server::new(vec![Response::json(json!({"version": "test"}))]);
    let output = isolated_command()
        .arg("--config")
        .arg(&path)
        .env("COLLAR_TOKEN", "test-token")
        .args(["--host", &other.origin, "api", "version"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let requests = other.finish();
    assert!(!requests[0].to_lowercase().contains("authorization:"));
}

#[test]
fn inferred_remote_overrides_saved_host_without_receiving_its_token() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("config.toml");
    std::fs::write(&path, "host = 'saved.example'\n").unwrap();
    let server = Server::new(vec![Response::json(json!({
        "full_name": "ScottyLabs/test", "description": null,
        "html_url": "https://example.test/test", "default_branch": "main",
        "private": false, "archived": false
    }))]);
    for args in [
        vec!["init", "--quiet"],
        vec![
            "remote",
            "add",
            "origin",
            &format!("{}/ScottyLabs/test.git", server.origin),
        ],
    ] {
        assert!(
            git_command()
                .args(args)
                .current_dir(temp.path())
                .status()
                .unwrap()
                .success()
        );
    }
    let output = isolated_command()
        .arg("--config")
        .arg(&path)
        .env("COLLAR_TOKEN", "saved-host-token")
        .current_dir(temp.path())
        .args(["repo", "view", "--json", "nameWithOwner"])
        .output()
        .unwrap();
    success(&output);
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        json!({"nameWithOwner": "ScottyLabs/test"})
    );
    let requests = server.finish();
    assert!(requests[0].starts_with("GET /api/v1/repos/ScottyLabs/test "));
    assert!(!requests[0].to_lowercase().contains("authorization:"));
}

#[test]
fn explicit_checkouts_and_host_environment_are_respected() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("config.toml");
    std::fs::write(&path, "host = 'saved.example'\n").unwrap();
    let checkout = temp.path().join("infra");
    std::fs::create_dir_all(checkout.join("modules/hosts/test")).unwrap();
    std::fs::create_dir_all(checkout.join("modules/roles")).unwrap();
    std::fs::write(checkout.join("modules/hosts/test/configuration.nix"), "{}").unwrap();
    std::fs::write(checkout.join("modules/roles/test.nix"), "{}").unwrap();
    let output = isolated_command()
        .env("COLLAR_CONFIG", &path)
        .env("COLLAR_INFRASTRUCTURE", &checkout)
        .args(["infra", "hosts", "--json", "name"])
        .output()
        .unwrap();
    success(&output);
    let output = isolated_command()
        .arg("--config")
        .arg(&path)
        .env("COLLAR_INFRASTRUCTURE", "missing")
        .args(["infra", "hosts", "--checkout"])
        .arg(&checkout)
        .args(["--json", "name"])
        .output()
        .unwrap();
    success(&output);
    let server = Server::new(vec![Response::json(json!({"version": "test"}))]);
    let output = isolated_command()
        .arg("--config")
        .arg(&path)
        .env("COLLAR_HOST", &server.origin)
        .env("COLLAR_TOKEN", "env-host-token")
        .args(["api", "version"])
        .output()
        .unwrap();
    success(&output);
    assert!(
        server.finish()[0]
            .to_lowercase()
            .contains("authorization: token env-host-token")
    );
    let original = std::fs::read_to_string(&path).unwrap();
    let output = isolated_command()
        .arg("--config")
        .arg(&path)
        .args(["config", "set", "governance"])
        .arg(&checkout)
        .output()
        .unwrap();
    failure(&output, "invalid value 'governance'");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
}
