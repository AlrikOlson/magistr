use super::*;

#[test]
fn completed_tool_fields() {
    let tool = CompletedTool {
        tool_use_id: "t1".into(),
        name: "Bash".into(),
        detail: "ls -la".into(),
        input: serde_json::json!({"command": "ls -la"}),
        output: Some("file.txt\n".into()),
        is_error: false,
        kind_icon: Some("⚡".into()),
    };
    assert_eq!(tool.name, "Bash");
    assert!(!tool.is_error);
    assert!(tool.output.is_some());
}

#[test]
fn completed_tool_error() {
    let tool = CompletedTool {
        tool_use_id: "t2".into(),
        name: "Edit".into(),
        detail: "src/main.rs".into(),
        input: serde_json::json!({}),
        output: None,
        is_error: true,
        kind_icon: None,
    };
    assert!(tool.is_error);
    assert!(tool.output.is_none());
    assert!(tool.kind_icon.is_none());
}

#[test]
fn max_completed_tools_constant() {
    assert_eq!(MAX_COMPLETED_TOOLS, 100);
}
