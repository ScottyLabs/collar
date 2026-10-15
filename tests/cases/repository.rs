use super::*;

#[test]
fn repository_fork_creates_a_copy_with_native_options() {
    let server = Server::new(vec![Response::json(json!({
        "full_name": "contributor/collar-copy",
        "html_url": "https://git.cmu.dev/contributor/collar-copy"
    }))]);
    let output = server
        .command(true)
        .args([
            "repo",
            "fork",
            "-R",
            "ScottyLabs/collar",
            "--name",
            "collar-copy",
        ])
        .output()
        .unwrap();
    success(&output);
    let requests = server.finish();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].starts_with("POST /api/v1/repos/ScottyLabs/collar/forks "));
    let body: Value = serde_json::from_str(requests[0].split_once("\r\n\r\n").unwrap().1).unwrap();
    assert_eq!(body, json!({"name": "collar-copy"}));
}

#[test]
fn provisioned_configuration_commands_are_not_exposed() {
    for arguments in [
        vec!["repo", "admin", "create", "scratch"],
        vec!["governance", "changes", "join"],
        vec!["kennel", "docs", "list"],
        vec!["governance", "run"],
        vec!["governance", "validate"],
        vec!["infra", "colmena"],
        vec!["changelog", "new"],
    ] {
        let output = command()
            .args(["--host", "http://127.0.0.1:1"])
            .args(arguments)
            .output()
            .unwrap();
        failure(&output, "unrecognized subcommand");
        assert_eq!(output.status.code(), Some(2));
    }
}

#[test]
fn broken_upstream_remote_does_not_silently_use_origin() {
    let directory = TempDir::new().unwrap();
    for args in [
        vec!["init", "--quiet"],
        vec![
            "config",
            "remote.upstream.fetch",
            "+refs/heads/*:refs/remotes/upstream/*",
        ],
        vec![
            "remote",
            "add",
            "origin",
            "https://git.cmu.dev/ScottyLabs/terrier.git",
        ],
    ] {
        assert!(
            git_command()
                .args(args)
                .current_dir(directory.path())
                .status()
                .unwrap()
                .success()
        );
    }
    let output = command()
        .current_dir(directory.path())
        .args(["repo", "view"])
        .output()
        .unwrap();
    failure(&output, "configured remote upstream");
}
