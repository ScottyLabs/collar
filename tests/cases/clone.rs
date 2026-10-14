use super::*;

#[test]
fn repo_clone_uses_existing_git_transport_without_forwarding_api_token() {
    for https in [false, true] {
        let temp = TempDir::new().unwrap();
        let bare = temp.path().join("source.git");
        assert!(
            Command::new("git")
                .args(["init", "--bare", "-q"])
                .arg(&bare)
                .status()
                .unwrap()
                .success()
        );
        let server = Server::with_responses(|origin| {
            vec![Response::json(json!({
                "ssh_url":"ssh://forgejo@127.0.0.1/ScottyLabs/terrier.git",
                "clone_url":format!("{origin}/ScottyLabs/terrier.git")
            }))]
        });
        let remote = if https {
            format!("{}/ScottyLabs/terrier.git", server.origin)
        } else {
            "ssh://forgejo@127.0.0.1/ScottyLabs/terrier.git".into()
        };
        let destination = temp.path().join("checkout");
        let mut command = server.command(false);
        command
            .args(["repo", "clone", "ScottyLabs/terrier"])
            .arg(&destination)
            .env("GIT_CONFIG_COUNT", "1")
            .env(
                "GIT_CONFIG_KEY_0",
                format!("url.file://{}.insteadOf", bare.display()),
            )
            .env("GIT_CONFIG_VALUE_0", &remote);
        if https {
            command.arg("--https");
        }
        let output = command.output().unwrap();
        success(&output);
        assert!(destination.join(".git").is_dir());
        let output = Command::new("git")
            .current_dir(&destination)
            .args(["config", "--get", "remote.origin.url"])
            .output()
            .unwrap();
        assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), remote);
        assert_eq!(server.finish().len(), 1);
    }
}

#[test]
fn repo_clone_refuses_an_unrelated_server_supplied_url() {
    let server = Server::new(vec![Response::json(json!({
        "ssh_url":"ssh://forgejo@other.invalid/ScottyLabs/terrier.git",
        "clone_url":"https://other.invalid/ScottyLabs/terrier.git"
    }))]);
    let output = server
        .command(false)
        .args(["repo", "clone", "ScottyLabs/terrier"])
        .output()
        .unwrap();
    failure(&output, "clone URL does not match");
    assert_eq!(server.finish().len(), 1);
}
