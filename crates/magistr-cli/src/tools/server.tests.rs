use super::*;
use tempfile::TempDir;

fn test_binary() -> PathBuf {
    PathBuf::from("/usr/local/bin/magistr")
}

#[test]
fn creates_new_mcp_config() {
    let dir = TempDir::new().unwrap();
    let guard = write_mcp_config(dir.path(), &test_binary()).unwrap();
    assert_eq!(guard.action, McpConfigAction::Created);
    assert!(guard.path.exists());

    let content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&guard.path).unwrap()).unwrap();
    assert!(
        content["mcpServers"]["magistr"]["command"]
            .as_str()
            .unwrap()
            .contains("magistr")
    );

    // Cleanup removes the file
    guard.cleanup();
    // Guard already cleaned up, but drop is idempotent
}

#[test]
fn merges_into_existing_config() {
    let dir = TempDir::new().unwrap();
    let mcp_path = dir.path().join(".mcp.json");
    std::fs::write(
        &mcp_path,
        r#"{"mcpServers": {"other": {"type": "stdio", "command": "other-tool"}}}"#,
    )
    .unwrap();

    let guard = write_mcp_config(dir.path(), &test_binary()).unwrap();
    assert_eq!(guard.action, McpConfigAction::Merged);

    let content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&guard.path).unwrap()).unwrap();
    // Both entries should exist
    assert!(content["mcpServers"]["other"].is_object());
    assert!(content["mcpServers"]["magistr"].is_object());
}

#[test]
fn leaves_existing_magistr_entry_unchanged() {
    let dir = TempDir::new().unwrap();
    let mcp_path = dir.path().join(".mcp.json");
    std::fs::write(
        &mcp_path,
        r#"{"mcpServers": {"magistr": {"type": "stdio", "command": "existing-magistr"}}}"#,
    )
    .unwrap();

    let guard = write_mcp_config(dir.path(), &test_binary()).unwrap();
    assert_eq!(guard.action, McpConfigAction::Unchanged);
}

#[test]
fn cleanup_removes_created_file() {
    let dir = TempDir::new().unwrap();
    let guard = write_mcp_config(dir.path(), &test_binary()).unwrap();
    let path = guard.path.clone();
    assert!(path.exists());
    drop(guard);
    assert!(!path.exists());
}

#[test]
fn cleanup_removes_merged_entry() {
    let dir = TempDir::new().unwrap();
    let mcp_path = dir.path().join(".mcp.json");
    std::fs::write(
        &mcp_path,
        r#"{"mcpServers": {"other": {"type": "stdio", "command": "other"}}}"#,
    )
    .unwrap();

    let guard = write_mcp_config(dir.path(), &test_binary()).unwrap();
    drop(guard);

    // File should still exist but magistr entry should be gone
    let content: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&mcp_path).unwrap()).unwrap();
    assert!(content["mcpServers"]["magistr"].is_null());
    assert!(content["mcpServers"]["other"].is_object());
}
