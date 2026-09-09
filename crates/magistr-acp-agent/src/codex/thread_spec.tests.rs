use super::*;
use agent_client_protocol::schema::v1::{EnvVariable, McpServerStdio, SessionId};

fn session(cwd: &std::path::Path) -> SessionStart {
    SessionStart {
        session_id: SessionId::new("persisted-thread"),
        cwd: cwd.to_path_buf(),
        mcp_servers: Vec::new(),
        resume: false,
    }
}

#[test]
fn new_thread_forwards_its_cwd_and_both_supported_mcp_transports() {
    let directory = tempfile::tempdir().expect("session directory");
    let mut start = session(directory.path());
    start.mcp_servers = vec![
        McpServer::Stdio(
            McpServerStdio::new("local-tools", "/fixture/runner")
                .args(vec!["serve".into(), "argument with spaces".into()])
                .env(vec![EnvVariable::new("FIXTURE_VALUE", "value with spaces")]),
        ),
        serde_json::from_value(json!({
            "type": "http", "name": "remote-tools", "url": "https://example.invalid/mcp",
            "headers": [{"name": "X-Fixture", "value": "opaque-value"}]
        }))
        .expect("HTTP MCP server"),
    ];
    let params = ThreadSpec::params(&start, "chosen-model").expect("thread params");
    assert_eq!(params["cwd"], json!(directory.path()));
    assert_eq!(params["model"], "chosen-model");
    assert!(
        params.get("threadId").is_none(),
        "the runtime mints a new id"
    );
    let servers = &params["config"]["mcp_servers"];
    assert_eq!(servers["local-tools"]["command"], "/fixture/runner");
    assert_eq!(
        servers["local-tools"]["args"],
        json!(["serve", "argument with spaces"])
    );
    assert_eq!(
        servers["local-tools"]["env"]["FIXTURE_VALUE"],
        "value with spaces"
    );
    assert_eq!(
        servers["remote-tools"]["url"],
        "https://example.invalid/mcp"
    );
    assert_eq!(
        servers["remote-tools"]["http_headers"],
        json!({"X-Fixture": "opaque-value"})
    );
    assert_eq!(params["approvalPolicy"], "untrusted");
    assert_eq!(params["approvalsReviewer"], "user");
    assert_eq!(params["sandbox"], "workspace-write");
    assert_eq!(params["modelProvider"], "openai");
}

#[test]
fn resume_names_the_persisted_thread_and_requests_history_without_changing_model() {
    let directory = tempfile::tempdir().expect("session directory");
    let mut start = session(directory.path());
    start.resume = true;
    let params = ThreadSpec::params(&start, "new-session-default").expect("resume params");
    assert_eq!(params["threadId"], "persisted-thread");
    assert_eq!(params["excludeTurns"], false);
    assert_eq!(params["cwd"], json!(directory.path()));
    assert!(
        params.get("model").is_none(),
        "resume preserves the runtime model"
    );
}

#[test]
fn duplicate_server_names_are_refused_instead_of_overwriting_the_first_server() {
    let directory = tempfile::tempdir().expect("session directory");
    let mut start = session(directory.path());
    start.mcp_servers = ["/fixture/first", "/fixture/second"]
        .into_iter()
        .map(|command| McpServer::Stdio(McpServerStdio::new("same-name", command)))
        .collect();
    let error = ThreadSpec::params(&start, "model").expect_err("ambiguous tool configuration");
    assert!(error.to_string().contains("duplicate") && error.to_string().contains("same-name"));
}

#[test]
fn legacy_sse_is_an_explicit_error_instead_of_silently_dropping_tools() {
    let directory = tempfile::tempdir().expect("session directory");
    let mut start = session(directory.path());
    start.mcp_servers.push(serde_json::from_value(json!({
        "type": "sse", "name": "legacy-tools", "url": "https://example.invalid/sse", "headers": []
    })).expect("SSE MCP server"));
    let error = ThreadSpec::params(&start, "model").expect_err("unsupported transport");
    assert!(error.to_string().contains("legacy-tools") && error.to_string().contains("SSE"));
}
