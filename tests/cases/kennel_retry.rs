use super::*;

const SHA: &str = "abcdef0123456789abcdef0123456789abcdef01";

fn branch(name: &str, sha: &str) -> Response {
    Response::json(json!({ "name": name, "commit": { "id": sha } }))
}

fn hook(id: u64) -> Value {
    json!({
        "id": id,
        "type": "forgejo",
        "active": true,
        "events": ["push", "pull_request"],
        "config": {
            "url": "https://kennel.scottylabs.org/webhook?namespace=terrier",
            "content_type": "json",
            "secret": "never-print-hook-secret",
        },
    })
}

fn prefix(name: &str) -> Vec<Response> {
    vec![
        Response::json(json!({ "version": "16.0.5" })),
        branch(name, SHA),
    ]
}

fn prepared(name: &str) -> Vec<Response> {
    let mut responses = prefix(name);
    responses.extend([
        Response::json(json!([hook(7)])),
        Response::json(json!([])),
        Response::json(hook(7)),
        branch(name, SHA),
    ]);
    responses
}

fn accepted() -> Response {
    Response {
        status: 204,
        body: String::new(),
        headers: String::new(),
    }
}

#[test]
fn retries_default_or_explicit_branch_with_one_targeted_push_test() {
    for name in [None, Some("staging"), Some("dev")] {
        let target = name.unwrap_or("main");
        let mut responses = if name.is_none() {
            vec![Response::json(json!({ "default_branch": "main" }))]
        } else {
            vec![]
        };
        responses.extend(prepared(target));
        responses.extend([accepted(), branch(target, SHA)]);
        let server = Server::new(responses);
        let mut command = server.command(true);
        command.args(["kennel", "retry"]);
        if let Some(name) = name {
            command.arg(name);
        }
        let output = command.args(["--json"]).output().unwrap();
        success(&output);
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["branch"], target);
        assert_eq!(report["headRefOid"], SHA);
        assert_eq!(report["triggerAccepted"], true);
        assert_eq!(report["headUnchanged"], true);
        for field in [
            "revisionVerified",
            "deliveryConfirmed",
            "buildStartConfirmed",
        ] {
            assert_eq!(report[field], false);
        }
        let requests = server.finish();
        let writes: Vec<_> = requests
            .iter()
            .filter(|request| request.starts_with("POST "))
            .collect();
        assert_eq!(writes.len(), 1);
        assert!(writes[0].starts_with(&format!(
            "POST /api/v1/repos/ScottyLabs/terrier/hooks/7/tests?ref=refs%2Fheads%2F{target} "
        )));
        assert!(
            !requests
                .iter()
                .any(|request| request.contains("/actions/") || request.contains("/git/commits"))
        );
        assert!(!String::from_utf8_lossy(&output.stdout).contains("never-print-hook-secret"));
    }
}

#[test]
fn rejects_missing_or_ambiguous_kennel_hooks_without_writes() {
    for hooks in [vec![], vec![hook(7), hook(8)]] {
        let mut responses = prefix("main");
        responses.push(Response::json(json!(hooks)));
        if !hooks.is_empty() {
            responses.push(Response::json(json!([])));
        }
        let server = Server::new(responses);
        let output = server
            .command(true)
            .args(["kennel", "retry", "main"])
            .output()
            .unwrap();
        failure(
            &output,
            if hooks.is_empty() {
                "no canonical Kennel webhook"
            } else {
                "--hook ID"
            },
        );
        assert!(
            server
                .finish()
                .iter()
                .all(|request| request.starts_with("GET "))
        );
    }
}

#[test]
fn explicit_hook_selects_only_a_verified_kennel_destination() {
    let mut responses = prefix("main");
    responses.extend([
        Response::json(json!([hook(7), hook(8)])),
        Response::json(json!([])),
        Response::json(hook(8)),
        branch("main", SHA),
        accepted(),
        branch("main", SHA),
    ]);
    let server = Server::new(responses);
    let output = server
        .command(true)
        .args(["kennel", "retry", "main", "--hook", "8"])
        .output()
        .unwrap();
    success(&output);
    assert!(server.finish().iter().any(|request| {
        request.starts_with("POST /api/v1/repos/ScottyLabs/terrier/hooks/8/tests?")
    }));
}

#[test]
fn refuses_disabled_wrong_payload_and_unrelated_hooks_even_with_explicit_id() {
    for change in [
        json!({ "active": false }),
        json!({ "url": "https://elsewhere.example/webhook" }),
        json!({ "type": "gitea" }),
        json!({ "events": ["pull_request"] }),
        json!({
            "config": { "url": "https://elsewhere.example/webhook", "content_type": "json" },
        }),
        json!({
            "config": {
                "url": "https://kennel.scottylabs.org/webhook",
                "content_type": "form",
            },
        }),
    ] {
        let mut value = hook(7);
        value
            .as_object_mut()
            .unwrap()
            .extend(change.as_object().unwrap().clone());
        let mut responses = prefix("main");
        responses.extend([Response::json(json!([value])), Response::json(json!([]))]);
        let server = Server::new(responses);
        let output = server
            .command(true)
            .args(["kennel", "retry", "main", "--hook", "7"])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(
            server
                .finish()
                .iter()
                .all(|request| request.starts_with("GET "))
        );
    }
}

#[test]
fn refuses_a_changed_branch_or_webhook_before_sending() {
    for changed_hook in [false, true] {
        let mut responses = prefix("main");
        responses.extend([Response::json(json!([hook(7)])), Response::json(json!([]))]);
        if changed_hook {
            let mut changed = hook(7);
            changed["config"]["url"] =
                json!("https://kennel.scottylabs.org/webhook?namespace=other");
            responses.push(Response::json(changed));
        } else {
            responses.extend([Response::json(hook(7)), branch("main", &"b".repeat(40))]);
        }
        let server = Server::new(responses);
        let output = server
            .command(true)
            .args(["kennel", "retry", "main"])
            .output()
            .unwrap();
        failure(&output, "changed while preparing retry");
        assert!(
            server
                .finish()
                .iter()
                .all(|request| request.starts_with("GET "))
        );
    }
}

#[test]
fn accepted_trigger_is_reported_even_when_revision_recheck_fails() {
    for denied in [false, true] {
        let mut responses = prepared("main");
        responses.push(accepted());
        responses.push(if denied {
            Response {
                status: 403,
                body: "denied".into(),
                headers: String::new(),
            }
        } else {
            branch("main", &"b".repeat(40))
        });
        let server = Server::new(responses);
        let output = server
            .command(true)
            .args(["kennel", "retry", "main", "--json"])
            .output()
            .unwrap();
        failure(&output, "inspect deliveries before another retry");
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["triggerAccepted"], true);
        assert_eq!(
            report["headUnchanged"],
            if denied { Value::Null } else { json!(false) }
        );
        assert_eq!(
            server
                .finish()
                .iter()
                .filter(|request| request.starts_with("POST "))
                .count(),
            1
        );
    }
}

#[test]
fn permission_and_uncertain_trigger_failures_are_actionable_and_never_retried() {
    for during_write in [false, true] {
        let mut responses = if during_write {
            prepared("main")
        } else {
            prefix("main")
        };
        responses.push(Response {
            status: if during_write { 500 } else { 403 },
            body: "never-print-server-secret".into(),
            headers: String::new(),
        });
        let server = Server::new(responses);
        let output = server
            .command(true)
            .args(["kennel", "retry", "main"])
            .output()
            .unwrap();
        failure(
            &output,
            if during_write {
                "inspect Forgejo deliveries"
            } else {
                "administrator permission"
            },
        );
        assert!(!String::from_utf8_lossy(&output.stderr).contains("never-print-server-secret"));
        assert_eq!(
            server
                .finish()
                .iter()
                .filter(|request| request.starts_with("POST "))
                .count(),
            usize::from(during_write)
        );
    }
}

#[test]
fn unsupported_pr_heads_and_branches_never_fall_back_to_default() {
    let server = Server::new(vec![Response::json(pull(SHA))]);
    let output = server
        .command(true)
        .args(["kennel", "retry", "--pr", "42"])
        .output()
        .unwrap();
    failure(&output, "previews require a pull-request event");
    assert_eq!(server.finish().len(), 1);
    for target in ["feature/example", "refs/tags/main", SHA] {
        let output = command()
            .env("COLLAR_HOST", "http://127.0.0.1:1")
            .env("COLLAR_TOKEN", "test-secret")
            .args(["-R", "terrier", "kennel", "retry", target])
            .output()
            .unwrap();
        failure(&output, "Kennel ignores push events");
    }
}

#[test]
fn invalid_output_and_missing_auth_fail_before_network() {
    for (extra, error) in [
        (vec!["--json", "badField"], "unsupported JSON field"),
        (vec![], "requires collar auth login"),
    ] {
        let output = command()
            .args([
                "--host",
                "http://127.0.0.1:1",
                "-R",
                "terrier",
                "kennel",
                "retry",
                "main",
            ])
            .args(extra)
            .output()
            .unwrap();
        failure(&output, error);
    }
}

#[test]
fn rejects_unverified_forgejo_versions_before_inspecting_or_triggering_hooks() {
    let server = Server::new(vec![Response::json(json!({ "version": "15.0.0" }))]);
    let output = server
        .command(true)
        .args(["kennel", "retry", "main"])
        .output()
        .unwrap();
    failure(&output, "Forgejo 16+");
    let requests = server.finish();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].starts_with("GET /api/v1/version "));
}
