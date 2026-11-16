use super::*;
use std::io::{BufRead, BufReader, Read};

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
    let expected = requests
        .iter()
        .filter(|request| request.get("id").is_some())
        .count();
    let mut reader = BufReader::new(child.stdout.take().unwrap());
    let mut responses = Vec::new();
    for _ in 0..expected {
        let mut line = String::new();
        assert!(reader.read_line(&mut line).unwrap() > 0);
        responses.push(serde_json::from_str(&line).unwrap());
    }
    drop(stdin);
    let output = child.wait_with_output().unwrap();
    success(&output);
    responses
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

#[test]
fn mcp_repository_view_uses_the_server_repository() {
    let server = Server::new(vec![Response::json(json!({
        "full_name":"ScottyLabs/terrier", "description":null,
        "html_url":"https://git.cmu.dev/ScottyLabs/terrier", "default_branch":"main", "private":false,"archived":false
    }))]);
    let mut command = server.command(true);
    command.arg("mcp");
    let responses = session(
        command,
        &[
            json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"repo_view","arguments":{}}}),
        ],
    );
    assert_eq!(responses[0]["result"]["isError"], false);
    assert!(server.finish()[0].starts_with("GET /api/v1/repos/ScottyLabs/terrier "));
}

struct McpProcess(std::process::Child);
impl Drop for McpProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn mcp_reaps_tools_on_cancellation_disconnect_and_server_termination() {
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};
    for shutdown in ["cancel", "disconnect", "kill"] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let mut command = command();
        command
            .env("COLLAR_HOST", &origin)
            .env("COLLAR_TOKEN", "test-secret")
            .arg("mcp");
        let mut child = McpProcess(
            command
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap(),
        );
        let mut input = child.0.stdin.take().unwrap();
        let output = child.0.stdout.take().unwrap();
        let (sender, receiver) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            for line in BufReader::new(output).lines() {
                if sender
                    .send(serde_json::from_str::<Value>(&line.unwrap()).unwrap())
                    .is_err()
                {
                    break;
                }
            }
        });
        writeln!(input,"{}",json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"api_get","arguments":{"endpoint":"version"}}})).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut socket = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "tool did not connect");
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("accept: {error}"),
            }
        };
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            assert_eq!(socket.read(&mut byte).unwrap(), 1);
            request.push(byte[0]);
        }
        // Ping must be handled while the HTTP tool is stalled.
        writeln!(input, "{}", json!({"jsonrpc":"2.0","id":2,"method":"ping"})).unwrap();
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(5)).unwrap()["id"],
            2
        );
        match shutdown {
            "cancel" => {
                writeln!(input,"{}",json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":1}})).unwrap();
                let response = receiver.recv_timeout(Duration::from_secs(5)).unwrap();
                assert_eq!(response["id"], 1);
                assert_eq!(response["result"]["isError"], true);
                assert!(
                    response["result"]["content"][0]["text"]
                        .as_str()
                        .unwrap()
                        .contains("cancelled")
                );
            }
            "kill" => {
                child.0.kill().unwrap();
            }
            _ => {}
        }
        drop(input);
        let mut byte = [0];
        match socket.read(&mut byte) {
            Ok(0) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::ConnectionAborted
                ) => {}
            result => panic!("tool connection survived {shutdown}: {result:?}"),
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        while child.0.try_wait().unwrap().is_none() {
            assert!(Instant::now() < deadline, "MCP server did not stop");
            std::thread::sleep(Duration::from_millis(10));
        }
        reader.join().unwrap();
    }
}

#[test]
fn malformed_mcp_calls_cannot_execute_write_tools() {
    let server = Server::new(vec![]);
    let mut command = server.command(true);
    command.args(["mcp", "--allow-writes"]);
    let mut requests = Vec::new();
    for id in [Value::Null, json!({"bad":"id"}), json!(1.5)] {
        requests.push(json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":"pr_review","arguments":{"selector":42,"event":"approve"}}}));
    }
    requests.push(json!({"jsonrpc":"1.0","id":5,"method":"tools/call","params":{"name":"pr_review","arguments":{"selector":42,"event":"approve"}}}));
    let responses = session(command, &requests);
    assert_eq!(responses.len(), 4);
    assert!(responses.iter().all(|r| r["error"]["code"] == -32600));
    assert!(server.finish().is_empty());
}

#[test]
fn mcp_issue_creation_resolves_label_arrays_through_the_cli() {
    let label = json!({"id":113,"name":"type/bug","color":"aabbcc","description":null,"url":null});
    let server = Server::new(vec![
        Response::json(json!([])),
        Response::json(json!([label.clone()])),
        Response::json(json!([])),
        Response::json(
            json!({"number":7,"title":"Bug","body":"Details","state":"open","html_url":"https://git.cmu.dev/ScottyLabs/terrier/issues/7","user":{"login":"kritdass"},"labels":[label],"comments":0}),
        ),
    ]);
    let mut command = server.command(true);
    command.args(["mcp", "--allow-writes"]);
    let responses = session(
        command,
        &[
            json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"issue_create","arguments":{"title":"Bug","body":"Details","labels":["type/bug"],"fields":"number,labels"}}}),
        ],
    );
    assert_eq!(responses[0]["result"]["isError"], false, "{responses:?}");
    let result: Value = serde_json::from_str(
        responses[0]["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(result["labels"][0]["name"], "type/bug");
    let requests = server.finish();
    assert_eq!(requests.len(), 4);
    assert!(requests[3].starts_with("POST /api/v1/repos/ScottyLabs/terrier/issues "));
    let body: Value = serde_json::from_str(requests[3].split_once("\r\n\r\n").unwrap().1).unwrap();
    assert_eq!(body, json!({"title":"Bug","body":"Details","labels":[113]}));
}
