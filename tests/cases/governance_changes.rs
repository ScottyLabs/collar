use super::*;
use std::os::unix::fs::PermissionsExt;

fn validator(temp: &TempDir, exit: u8) -> std::path::PathBuf {
    let bin = temp.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let cargo = bin.join("cargo");
    std::fs::write(&cargo, format!("#!/bin/sh\nexit {exit}\n")).unwrap();
    std::fs::set_permissions(&cargo, std::fs::Permissions::from_mode(0o755)).unwrap();
    bin
}

#[test]
fn feature_change_preserves_toml_comments_and_uses_local_checkout() {
    let temp = TempDir::new().unwrap();
    project_governance(temp.path());
    std::fs::write(temp.path().join("Cargo.toml"), "[workspace]\n").unwrap();
    let bin = validator(&temp, 0);
    let path = temp.path().join("data/teams/team.toml");
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str("\n# keep this operator note\n");
    std::fs::write(&path, text).unwrap();

    let output = command()
        .env(
            "PATH",
            format!("{}:{}", bin.display(), std::env::var("PATH").unwrap()),
        )
        .args([
            "-R",
            "ScottyLabs/collie-api",
            "governance",
            "changes",
            "enable",
            "--checkout",
        ])
        .arg(temp.path())
        .args(["--feature", "docs"])
        .output()
        .unwrap();
    success(&output);
    let updated = std::fs::read_to_string(&path).unwrap();
    assert!(updated.contains("# keep this operator note"));
    let document: toml::Value = toml::from_str(&updated).unwrap();
    let repositories = document["team"]["projects"][0]["repos"].as_array().unwrap();
    assert!(
        repositories[0]["features"]["docs"]
            .as_table()
            .unwrap()
            .is_empty()
    );
    assert!(repositories[1]["features"].get("docs").is_none());
}

#[test]
fn invalid_governance_change_restores_original_file() {
    let temp = TempDir::new().unwrap();
    project_governance(temp.path());
    std::fs::write(temp.path().join("Cargo.toml"), "[workspace]\n").unwrap();
    let bin = validator(&temp, 1);
    let path = temp.path().join("data/teams/team.toml");
    let original = std::fs::read_to_string(&path).unwrap();
    let output = command()
        .env(
            "PATH",
            format!("{}:{}", bin.display(), std::env::var("PATH").unwrap()),
        )
        .args([
            "-R",
            "ScottyLabs/collie-api",
            "governance",
            "changes",
            "enable",
            "--checkout",
        ])
        .arg(temp.path())
        .args(["--feature", "docs"])
        .output()
        .unwrap();
    failure(&output, "original file was restored");
    assert_eq!(std::fs::read_to_string(path).unwrap(), original);
}

#[test]
fn self_membership_join_is_idempotent_and_leave_removes_only_self() {
    let temp = TempDir::new().unwrap();
    project_governance(temp.path());
    let org = temp.path().join("data/org.toml");
    let server = Server::new(vec![
        Response::json(json!({"login": "alice"})),
        Response::json(json!({"login": "alice"})),
        Response::json(json!({"login": "alice"})),
        Response::json(json!({"login": "lead"})),
    ]);
    let text = std::fs::read_to_string(&org).unwrap();
    std::fs::write(&org, text.replace("https://git.cmu.dev", &server.origin)).unwrap();
    std::fs::write(temp.path().join("Cargo.toml"), "[workspace]\n").unwrap();
    let bin = validator(&temp, 0);
    let team = temp.path().join("data/teams/team.toml");
    let invoke = |action: &str| {
        let mut child = command();
        child
            .env("COLLAR_HOST", &server.origin)
            .env("COLLAR_TOKEN", "test-secret")
            .env(
                "PATH",
                format!("{}:{}", bin.display(), std::env::var("PATH").unwrap()),
            )
            .args([
                "-R",
                "ScottyLabs/collie-api",
                "governance",
                "changes",
                action,
                "--checkout",
            ])
            .arg(temp.path())
            .args(["--team", "tech"]);
        child.output().unwrap()
    };

    let output = invoke("join");
    success(&output);
    let joined = std::fs::read_to_string(&team).unwrap();
    assert!(
        toml::from_str::<toml::Value>(&joined).unwrap()["team"]["members"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value.as_str() == Some("alice"))
    );

    success(&invoke("join"));
    assert_eq!(std::fs::read_to_string(&team).unwrap(), joined);

    success(&invoke("leave"));
    let left = std::fs::read_to_string(&team).unwrap();
    assert!(
        !toml::from_str::<toml::Value>(&left).unwrap()["team"]["members"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value.as_str() == Some("alice"))
    );

    let output = invoke("leave");
    failure(&output, "leads cannot remove their own lead role");
    assert_eq!(std::fs::read_to_string(&team).unwrap(), left);
    let requests = server.finish();
    assert_eq!(requests.len(), 4);
    assert!(
        requests
            .iter()
            .all(|request| request.starts_with("GET /api/v1/user "))
    );
    assert!(requests.iter().all(|request| {
        request
            .to_lowercase()
            .contains("authorization: token test-secret")
    }));
}
