//! End-to-end MCP integration tests.
//!
//! Spawns `magistr mcp-serve` as a subprocess and verifies the MCP protocol
//! works end-to-end: initialize handshake, tools/list, and tools/call.

#[path = "support/magistr_command.rs"]
mod magistr_command;

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

use tempfile::TempDir;

/// Helper: build the magistr binary path via cargo.
fn magistr_bin() -> Command {
    magistr_command::magistr_command()
}

/// Send a JSON-RPC request and read the response line.
fn send_request(
    stdin: &mut impl Write,
    stdout: &mut impl BufRead,
    request: &serde_json::Value,
) -> serde_json::Value {
    let msg = serde_json::to_string(request).unwrap();
    writeln!(stdin, "{msg}").unwrap();
    stdin.flush().unwrap();

    let mut response_line = String::new();
    stdout.read_line(&mut response_line).unwrap();
    serde_json::from_str(response_line.trim()).unwrap()
}

#[test]
fn mcp_serve_initialize_handshake() {
    let tmp = TempDir::new().unwrap();

    let mut child = magistr_bin()
        .arg("mcp-serve")
        .arg(tmp.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn magistr mcp-serve");

    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    // Send initialize
    let init_request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "test-client",
                "version": "0.1.0"
            }
        }
    });

    let response = send_request(&mut stdin, &mut stdout, &init_request);

    // Verify initialize response
    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], 1);
    let result = &response["result"];
    assert_eq!(result["protocolVersion"], "2024-11-05");
    assert_eq!(result["serverInfo"]["name"], "magistr");
    assert!(result["capabilities"]["tools"].is_object());

    // Close stdin to signal EOF and let the server exit
    drop(stdin);
    let status = child.wait().unwrap();
    assert!(status.success());
}

#[test]
fn mcp_serve_tools_list_returns_magistr_tools() {
    let tmp = TempDir::new().unwrap();

    let mut child = magistr_bin()
        .arg("mcp-serve")
        .arg(tmp.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn magistr mcp-serve");

    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    // Initialize first (required by MCP protocol)
    let init = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "test", "version": "0.1.0" }
        }
    });
    let _ = send_request(&mut stdin, &mut stdout, &init);

    // Send tools/list
    let tools_list = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {}
    });
    let response = send_request(&mut stdin, &mut stdout, &tools_list);

    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], 2);

    let tools = response["result"]["tools"]
        .as_array()
        .expect("tools should be an array");
    let tool_names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();

    assert!(
        tool_names.contains(&"magistr_delete"),
        "missing magistr_delete tool: {tool_names:?}"
    );
    assert!(
        tool_names.contains(&"magistr_list"),
        "missing magistr_list tool: {tool_names:?}"
    );

    // Verify tools have schemas
    for tool in tools {
        assert!(tool["name"].is_string());
        assert!(tool["description"].is_string());
        assert!(tool["inputSchema"].is_object());
    }

    drop(stdin);
    let status = child.wait().unwrap();
    assert!(status.success());
}

#[test]
fn mcp_serve_tools_call_magistr_list() {
    let tmp = TempDir::new().unwrap();
    // Create a test file to list
    std::fs::write(tmp.path().join("hello.txt"), "world").unwrap();
    std::fs::create_dir(tmp.path().join("subdir")).unwrap();

    let mut child = magistr_bin()
        .arg("mcp-serve")
        .arg(tmp.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn magistr mcp-serve");

    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    // Initialize
    let init = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05", "capabilities": {},
            "clientInfo": { "name": "test", "version": "0.1.0" }
        }
    });
    let _ = send_request(&mut stdin, &mut stdout, &init);

    // Call magistr_list on the project root
    let call = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "magistr_list",
            "arguments": { "path": "." }
        }
    });
    let response = send_request(&mut stdin, &mut stdout, &call);

    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], 3);

    // The result should contain content with the listed files
    let content = &response["result"]["content"];
    let text = content[0]["text"]
        .as_str()
        .expect("should have text content");
    assert!(
        text.contains("hello.txt"),
        "listing should include hello.txt: {text}"
    );
    assert!(
        text.contains("subdir/"),
        "listing should include subdir/: {text}"
    );

    drop(stdin);
    let status = child.wait().unwrap();
    assert!(status.success());
}

#[test]
fn mcp_proxy_filters_non_json_lines() {
    // mcp-proxy wraps a child process and filters stdout so only JSON-RPC
    // lines (starting with '{') reach the caller. We test this by proxying
    // `magistr mcp-serve` through `magistr mcp-proxy`.
    let tmp = TempDir::new().unwrap();

    let magistr_path = env!("CARGO_BIN_EXE_magistr");

    let mut child = magistr_command::magistr_command()
        .args(["mcp-proxy", "--", magistr_path, "mcp-serve"])
        .arg(tmp.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn magistr mcp-proxy");

    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    // Send initialize through the proxy
    let init = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "test", "version": "0.1.0" }
        }
    });
    let response = send_request(&mut stdin, &mut stdout, &init);

    // The proxy should pass through valid JSON-RPC unchanged
    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], 1);
    assert_eq!(response["result"]["serverInfo"]["name"], "magistr");

    drop(stdin);
    // mcp-proxy calls process::exit so we just kill it
    let _ = child.kill();
    let _ = child.wait();
}
