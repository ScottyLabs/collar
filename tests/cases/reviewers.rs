use super::*;

fn requests(users: &[&str]) -> Value {
    let mut value = pull("head");
    value["requested_reviewers"] = json!(
        users
            .iter()
            .map(|login| json!({"login":login}))
            .collect::<Vec<_>>()
    );
    value["requested_reviewers_teams"] = json!([{"name":"maintainers"}]);
    value
}

#[test]
fn lists_requested_users_and_teams_without_writing() {
    let server = Server::new(vec![Response::json(requests(&["anish"]))]);
    let output = server
        .command(false)
        .args(["pr", "reviewers", "42", "--json"])
        .output()
        .unwrap();
    success(&output);
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        json!({"number":42,"reviewers":["anish"],"teams":["maintainers"]})
    );
    assert!(server.finish()[0].starts_with("GET /api/v1/repos/ScottyLabs/terrier/pulls/42 "));
}

#[test]
fn reviewer_changes_are_one_explicit_write_followed_by_a_refresh() {
    for (flag, method) in [("--add", "POST"), ("--remove", "DELETE")] {
        let server = Server::new(vec![
            Response::json(requests(&[])),
            Response::json(json!([])),
            Response::json(requests(&["anish", "kritdass"])),
        ]);
        let output = server
            .command(true)
            .args([
                "pr",
                "reviewers",
                "42",
                flag,
                "anish",
                "kritdass",
                "--json",
                "reviewers",
            ])
            .output()
            .unwrap();
        success(&output);
        let requests = server.finish();
        assert_eq!(requests.len(), 3);
        assert!(requests[1].starts_with(&format!(
            "{method} /api/v1/repos/ScottyLabs/terrier/pulls/42/requested_reviewers "
        )));
        let body: Value =
            serde_json::from_str(requests[1].split_once("\r\n\r\n").unwrap().1).unwrap();
        assert_eq!(body, json!({"reviewers":["anish","kritdass"]}));
        assert!(requests[2].starts_with("GET "));
    }
}

#[test]
fn reviewer_changes_reject_conflicts_invalid_users_and_missing_auth_before_network() {
    for (args, message) in [
        (
            vec!["--add", "anish", "--remove", "kritdass"],
            "cannot be used with",
        ),
        (vec!["--add", "not a login"], "reviewer must be a username"),
        (vec!["--add", "anish"], "requires collar auth login"),
    ] {
        let output = command()
            .args([
                "--host",
                "http://127.0.0.1:1",
                "-R",
                "terrier",
                "pr",
                "reviewers",
                "42",
            ])
            .args(args)
            .output()
            .unwrap();
        failure(&output, message);
    }
}

#[test]
fn reviewer_changes_reject_closed_prs_without_writing() {
    let mut closed = requests(&[]);
    closed["state"] = json!("closed");
    let server = Server::new(vec![Response::json(closed)]);
    let output = server
        .command(true)
        .args(["pr", "reviewers", "42", "--add", "anish"])
        .output()
        .unwrap();
    failure(&output, "PR must be open");
    assert_eq!(server.finish().len(), 1);
}
