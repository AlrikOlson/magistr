use super::*;

/// A project directory holding `.mcp.json` with `contents`.
fn project(contents: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a temporary project directory");
    std::fs::write(dir.path().join(".mcp.json"), contents).expect(".mcp.json is written");
    dir
}

#[test]
fn a_project_with_no_mcp_config_has_no_workbench() {
    let dir = tempfile::tempdir().expect("a temporary project directory");
    assert!(workbench_port(dir.path()).is_none());
}

#[test]
fn a_config_without_the_workbench_server_has_no_workbench() {
    let dir = project(r#"{"mcpServers":{"external":{"command":"server"}}}"#);
    assert!(workbench_port(dir.path()).is_none());
}

#[test]
fn an_entry_without_a_command_has_no_workbench() {
    let dir = project(r#"{"mcpServers":{"magistr":{"args":["."]}}}"#);
    assert!(workbench_port(dir.path()).is_none());
}

#[test]
fn unparsable_json_has_no_workbench_rather_than_a_failure() {
    let dir = project("{ not json");
    assert!(workbench_port(dir.path()).is_none());
}

#[test]
fn the_configured_server_becomes_a_port() {
    let dir = project(
        r#"{"mcpServers":{"magistr":{"command":"bun","args":["run","main.ts","/repo"],"env":{"X":"1"}}}}"#,
    );
    assert!(workbench_port(dir.path()).is_some());
}

#[test]
fn building_the_port_never_starts_the_server() {
    // The command does not exist; the port is still built, because the
    // connection is not made until something is asked of it.
    let dir = project(r#"{"mcpServers":{"magistr":{"command":"magistr-no-such-server"}}}"#);
    let mut port = workbench_port(dir.path()).expect("the entry becomes a port");
    // Only now does it try, and the refusal is words rather than a panic.
    assert!(port.map().is_err());
}
