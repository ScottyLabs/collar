use super::*;
use std::process::Stdio;

fn login_command(server: &Server, temp: &TempDir) -> Command {
    let mut command = isolated_command();
    command
        .env("COLLAR_HOST", &server.origin)
        .arg("--config")
        .arg(temp.path().join("config.toml"))
        .args(["auth", "login"]);
    command
}

fn submit(mut command: Command, token: &str) -> Output {
    let mut child = command
        .arg("--with-token")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(token.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn rejected_login_does_not_save_metadata_or_echo_token() {
    let temp = TempDir::new().unwrap();
    let server = Server::new(vec![Response {
        status: 401,
        body: "rejected input-test-token".into(),
        headers: String::new(),
    }]);
    let mut command = login_command(&server, &temp);
    command.env("COLLAR_TOKEN", "different-env-token");
    let output = submit(command, "input-test-token\n");
    failure(&output, "HTTP 401");
    assert!(!temp.path().join("config.toml").exists());
    assert!(!String::from_utf8_lossy(&output.stderr).contains("input-test-token"));
    let requests = server.finish();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].starts_with("GET /api/v1/user "));
    assert!(requests[0].contains("input-test-token"));
    assert!(!requests[0].contains("different-env-token"));
}

#[test]
fn login_validates_input_and_headless_intent_before_network_access() {
    let temp = TempDir::new().unwrap();
    let server = Server::new(vec![]);
    let output = login_command(&server, &temp).output().unwrap();
    failure(&output, "use --with-token");
    for token in [String::new(), "one\ntwo".into(), "x".repeat(65537)] {
        let output = submit(login_command(&server, &temp), &token);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
    let output = command()
        .args(["auth", "login", "--with-token"])
        .output()
        .unwrap();
    failure(&output, "configuration disabled");
    assert!(server.finish().is_empty());
    assert!(!temp.path().join("config.toml").exists());
}

#[cfg(target_os = "linux")]
#[test]
fn unavailable_os_store_does_not_save_plaintext_or_login_metadata() {
    let temp = TempDir::new().unwrap();
    let server = Server::new(vec![Response::json(json!({"login":"fixture-user"}))]);
    let mut command = login_command(&server, &temp);
    command.env(
        "DBUS_SESSION_BUS_ADDRESS",
        format!("unix:path={}", temp.path().join("absent-bus").display()),
    );
    let output = submit(command, "offline-store-token");
    failure(&output, "OS credential store operation failed");
    assert!(!String::from_utf8_lossy(&output.stderr).contains("offline-store-token"));
    assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 0);
    assert_eq!(server.finish().len(), 1);
}
