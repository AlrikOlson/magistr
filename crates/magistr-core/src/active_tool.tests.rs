use std::time::Instant;

use super::*;

#[test]
fn active_tool_creation() {
    let tool = ActiveTool {
        name: "Bash".into(),
        detail: "cargo test".into(),
        started: Instant::now(),
        kind: Some(ToolKind::Execute),
    };
    assert_eq!(tool.name, "Bash");
    assert_eq!(tool.kind, Some(ToolKind::Execute));
}

#[test]
fn active_tool_no_kind() {
    let tool = ActiveTool {
        name: "Custom".into(),
        detail: "doing something".into(),
        started: Instant::now(),
        kind: None,
    };
    assert!(tool.kind.is_none());
}
