use agent_client_protocol::schema::v1::{EnvVariable, McpServer, McpServerStdio};

use super::*;

fn spec() -> LaunchSpec {
    LaunchSpec {
        cwd: PathBuf::from("/work"),
        session_id: "0d2b7a4e-1111-4222-8333-444455556666".to_owned(),
        resume: false,
        model: None,
        mcp_servers: Vec::new(),
        allowed_tools: Vec::new(),
    }
}

#[test]
fn allowed_tools_become_one_comma_joined_allowed_tools_flag() {
    let mut s = spec();
    assert!(!s.args().contains(&"--allowedTools".to_owned()));
    s.allowed_tools = vec!["Bash(git *)".to_owned(), "Edit".to_owned()];
    let args = s.args();
    let at = args
        .iter()
        .position(|a| a == "--allowedTools")
        .expect("--allowedTools");
    assert_eq!(args[at + 1], "Bash(git *),Edit");
}

#[test]
fn a_bare_mcp_tool_name_is_expanded_to_every_session_server() {
    let mut s = spec();
    s.mcp_servers = vec![
        McpServer::Stdio(McpServerStdio::new("magistr", "magistr")),
        McpServer::Stdio(McpServerStdio::new("workbench-mcp", "workbench-mcp")),
    ];
    s.allowed_tools = vec![
        "Bash".to_owned(),
        "roadmap_status".to_owned(),
        "mcp__magistr__magistr_gate".to_owned(),
    ];
    let args = s.args();
    let at = args
        .iter()
        .position(|a| a == "--allowedTools")
        .expect("--allowedTools");
    assert_eq!(
        args[at + 1],
        "Bash,roadmap_status,mcp__magistr__roadmap_status,mcp__workbench-mcp__roadmap_status,mcp__magistr__magistr_gate"
    );
}

#[test]
fn a_new_session_on_the_default_model_gets_the_base_flags_and_its_id() {
    let args = spec().args();
    let base: Vec<&str> = args
        .iter()
        .take(LaunchSpec::BASE_ARGS.len())
        .map(String::as_str)
        .collect();
    assert_eq!(base, LaunchSpec::BASE_ARGS);
    assert!(args.contains(&"--permission-mode".to_owned()));
    assert!(args.contains(&"--strict-mcp-config".to_owned()));
    assert!(!args.contains(&"--model".to_owned()), "{args:?}");
    assert!(!args.contains(&"--mcp-config".to_owned()), "{args:?}");
    assert_eq!(
        &args[args.len() - 2..],
        ["--session-id", "0d2b7a4e-1111-4222-8333-444455556666"]
    );
}

#[test]
fn the_default_model_id_is_never_passed_but_a_real_one_is() {
    let mut s = spec();
    s.model = Some(LaunchSpec::DEFAULT_MODEL.to_owned());
    assert!(!s.args().contains(&"--model".to_owned()));
    s.model = Some("claude-fable-5-1[1m]".to_owned());
    let args = s.args();
    let at = args.iter().position(|a| a == "--model").expect("--model");
    assert_eq!(args[at + 1], "claude-fable-5-1[1m]");
}

#[test]
fn a_resumed_session_uses_resume_instead_of_session_id() {
    let mut s = spec();
    s.resume = true;
    let args = s.args();
    assert!(!args.contains(&"--session-id".to_owned()));
    assert_eq!(
        &args[args.len() - 2..],
        ["--resume", "0d2b7a4e-1111-4222-8333-444455556666"]
    );
}

#[test]
fn stdio_servers_become_the_mcp_config_document() {
    let mut s = spec();
    s.mcp_servers = vec![McpServer::Stdio(
        McpServerStdio::new("magistr", "/usr/local/bin/magistr")
            .args(vec!["mcp-serve".to_owned()])
            .env(vec![EnvVariable::new("MAGISTR_LOG", "debug")]),
    )];
    let args = s.args();
    let at = args
        .iter()
        .position(|a| a == "--mcp-config")
        .expect("--mcp-config");
    let doc: serde_json::Value = serde_json::from_str(&args[at + 1]).expect("json");
    assert_eq!(
        doc,
        serde_json::json!({"mcpServers": {"magistr": {
            "command": "/usr/local/bin/magistr",
            "args": ["mcp-serve"],
            "env": {"MAGISTR_LOG": "debug"},
        }}})
    );
}
