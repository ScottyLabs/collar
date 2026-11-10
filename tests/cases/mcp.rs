use super::*;

fn session(mut command: Command, requests: &[Value]) -> Vec<Value> {
    let mut child = command
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    for request in requests {
        writeln!(stdin, "{request}").unwrap();
    }
    drop(stdin);
    let output = child.wait_with_output().unwrap();
    success(&output);
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[test]
fn mcp_tools_run_collar_reads_and_hide_writes_by_default() {
    let server = Server::new(vec![Response::json(pull("abc123"))]);
    let mut command = server.command(true);
    command.arg("mcp");
    let responses = session(
        command,
        &[
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "test", "version": "0"}}}),
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
            json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "pr_view", "arguments": {"selector": 42, "fields": "number,title,headRefOid"}}}),
            json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "pr_review", "arguments": {"selector": 42, "event": "approve"}}}),
        ],
    );
    assert_eq!(responses.len(), 4, "notifications receive no response");
    assert_eq!(responses[0]["result"]["serverInfo"]["name"], "collar");
    let tools = responses[1]["result"]["tools"].as_array().unwrap();
    assert!(tools.iter().any(|tool| tool["name"] == "pr_view"));
    assert!(!tools.iter().any(|tool| tool["name"] == "pr_review"));

    let result = &responses[2]["result"];
    assert_eq!(result["isError"], false);
    let view: Value = serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(
        view,
        json!({"number": 42, "title": "Review me", "headRefOid": "abc123"})
    );
    assert_eq!(responses[3]["error"]["code"], -32602);

    let requests = server.finish();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].starts_with("GET /api/v1/repos/ScottyLabs/terrier/pulls/42 "));
}

#[test]
fn mcp_tool_failures_are_results_without_secrets() {
    let server = Server::new(vec![Response {
        status: 404,
        body: "echoed credential: test-secret".into(),
        headers: String::new(),
    }]);
    let mut command = server.command(true);
    command.arg("mcp");
    let responses = session(
        command,
        &[
            json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "issue_view", "arguments": {"number": 7}}}),
        ],
    );
    let result = &responses[0]["result"];
    assert_eq!(result["isError"], true);
    let text = result["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("404"), "{text}");
    assert!(!text.contains("test-secret"));
    server.finish();
}
