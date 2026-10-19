use super::*;
use serde_json::json;

#[test]
fn stdio_servers_map_with_args_and_env_and_others_are_skipped() {
    let doc = json!({
        "mcpServers": {
            "magistr": { "type": "stdio", "command": "magistr", "args": ["mcp", "--dir", "/p"] },
            "think-and-ship": {
                "command": "think-and-ship", "args": ["serve"],
                "env": { "THINK_AND_SHIP_PERSIST": "true" }
            },
            "remote": { "type": "http", "url": "https://example.test/mcp" }
        }
    });
    let servers = mcp_servers_from_value(&doc);
    assert_eq!(servers.len(), 2, "{servers:?}");
    let stdio: Vec<&McpServerStdio> = servers
        .iter()
        .filter_map(|s| match s {
            McpServer::Stdio(s) => Some(s),
            _ => None,
        })
        .collect();
    let magistr = stdio.iter().find(|s| s.name == "magistr").expect("magistr");
    assert_eq!(magistr.command, std::path::PathBuf::from("magistr"));
    assert_eq!(magistr.args, vec!["mcp", "--dir", "/p"]);
    assert!(magistr.env.is_empty());
    let tas = stdio
        .iter()
        .find(|s| s.name == "think-and-ship")
        .expect("think-and-ship");
    assert_eq!(tas.env.len(), 1);
    assert_eq!(tas.env[0].name, "THINK_AND_SHIP_PERSIST");
    assert_eq!(tas.env[0].value, "true");
}

#[test]
fn missing_or_broken_file_yields_no_servers() {
    let tmp = tempfile::tempdir().expect("tempdir");
    assert!(project_mcp_servers(tmp.path()).is_empty());
    std::fs::write(tmp.path().join(".mcp.json"), "{ not json").expect("write");
    assert!(project_mcp_servers(tmp.path()).is_empty());
    std::fs::write(
        tmp.path().join(".mcp.json"),
        r#"{"mcpServers":{"only":{"command":"only-server"}}}"#,
    )
    .expect("write");
    assert_eq!(project_mcp_servers(tmp.path()).len(), 1);
}
