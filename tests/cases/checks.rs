use super::*;

#[test]
fn checks_use_the_latest_status_and_fail_for_pending_checks() {
    let server = Server::new(vec![
        Response::json(pull("head")),
        Response::json(json!([
            { "id": 1, "context": "ci", "status": "success" },
            { "id": 2, "context": "ci", "status": "pending" }
        ])),
        Response::json(json!([])),
    ]);
    let output = server
        .command(false)
        .args(["pr", "checks", "42", "--json", "name,state"])
        .output()
        .unwrap();
    failure(&output, "not successful");
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        json!([{ "name": "ci", "state": "pending" }])
    );
    server.finish();
}
