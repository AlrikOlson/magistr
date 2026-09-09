use super::*;

use magistr_engine::{LifecycleEvent, WorkflowPhase};
use tempfile::TempDir;

fn setup_hooks_project() -> (TempDir, std::path::PathBuf) {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_path_buf();
    let magistr_dir = root.join(".magistr");
    std::fs::create_dir_all(&magistr_dir).unwrap();
    std::fs::write(
        magistr_dir.join("project.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "project": { "name": "test", "description": "", "type": "cli" },
            "stack": {
                "language": "rust",
                "framework": "none",
                "package_manager": "cargo",
                "test_framework": "cargo-test",
                "linter": "clippy",
                "formatter": "rustfmt"
            },
            "quality_gate": {
                "check": "echo check",
                "test": "echo test",
                "lint": "echo lint"
            },
            "format": { "extensions": ["rs"], "command": "echo fmt" },
            "spec": { "file": "ROADMAP.md", "unchecked_pattern": "" },
            "testing": { "companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside" },
            "reference_repos": [],
            "state_prefix": "test"
        }))
        .unwrap(),
    )
    .unwrap();
    (tmp, root)
}

/// Extract text content from a tool result.
fn extract_text(result: &magistr_mcp::ToolResult) -> &str {
    match &result.content[0] {
        magistr_mcp::ToolContent::Text { text } => text,
        _ => panic!("expected text content"),
    }
}

// ── parse_event tests ───────────────────────────────────────────────

#[test]
fn parse_event_on_enter_research() {
    assert_eq!(
        parse_event("on_enter(research)"),
        Some(LifecycleEvent::OnEnter(WorkflowPhase::Researching))
    );
}

#[test]
fn parse_event_on_exit_verify() {
    assert_eq!(
        parse_event("on_exit(verify)"),
        Some(LifecycleEvent::OnExit(WorkflowPhase::Verifying))
    );
}

#[test]
fn parse_event_on_enter_done() {
    assert_eq!(
        parse_event("on_enter(done)"),
        Some(LifecycleEvent::OnEnter(WorkflowPhase::Complete))
    );
}

#[test]
fn parse_event_simple_events() {
    assert_eq!(
        parse_event("on_iteration_start"),
        Some(LifecycleEvent::OnIterationStart)
    );
    assert_eq!(
        parse_event("on_iteration_end"),
        Some(LifecycleEvent::OnIterationEnd)
    );
    assert_eq!(
        parse_event("on_gate_pass"),
        Some(LifecycleEvent::OnGatePass)
    );
    assert_eq!(
        parse_event("on_gate_fail"),
        Some(LifecycleEvent::OnGateFail)
    );
}

#[test]
fn parse_event_invalid() {
    assert_eq!(parse_event("invalid_event"), None);
    assert_eq!(parse_event("on_enter(invalid_phase)"), None);
    assert_eq!(parse_event("on_enter()"), None);
    assert_eq!(parse_event(""), None);
}

#[test]
fn parse_event_with_whitespace() {
    assert_eq!(
        parse_event("  on_gate_pass  "),
        Some(LifecycleEvent::OnGatePass)
    );
}

// ── MCP tool integration tests ──────────────────────────────────────

#[tokio::test]
async fn hooks_list_empty() {
    let (_tmp, root) = setup_hooks_project();
    let server = crate::mcp_tools::build_server(root);
    let tool = server.get_tool("magistr_hooks_list").unwrap();

    let result = tool.invoke(serde_json::json!({})).await.unwrap();
    let text = extract_text(&result);
    assert!(text.contains("No lifecycle hooks configured"));
}

#[tokio::test]
async fn hooks_add_then_list() {
    let (_tmp, root) = setup_hooks_project();
    let server = crate::mcp_tools::build_server(root);

    // Add a hook
    let add_tool = server.get_tool("magistr_hooks_add").unwrap();
    let result = add_tool
        .invoke(serde_json::json!({
            "event": "on_enter(verify)",
            "tool": "magistr_format",
            "description": "Auto-format before verify"
        }))
        .await
        .unwrap();
    let text = extract_text(&result);
    assert!(text.contains("Hook added"));
    assert!(text.contains("magistr_format"));

    // List should show the hook
    let list_tool = server.get_tool("magistr_hooks_list").unwrap();
    let result = list_tool.invoke(serde_json::json!({})).await.unwrap();
    let text = extract_text(&result);
    assert!(text.contains("magistr_format"));
    assert!(text.contains("Auto-format before verify"));
    assert!(text.contains("1 total"));
}

#[tokio::test]
async fn hooks_add_duplicate_rejected() {
    let (_tmp, root) = setup_hooks_project();
    let server = crate::mcp_tools::build_server(root);
    let add_tool = server.get_tool("magistr_hooks_add").unwrap();

    // First add succeeds
    let result = add_tool
        .invoke(serde_json::json!({
            "event": "on_gate_pass",
            "tool": "my_tool"
        }))
        .await
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));

    // Duplicate is rejected
    let result = add_tool
        .invoke(serde_json::json!({
            "event": "on_gate_pass",
            "tool": "my_tool"
        }))
        .await
        .unwrap();
    let text = extract_text(&result);
    assert!(text.contains("Duplicate"));
}

#[tokio::test]
async fn hooks_add_invalid_event_rejected() {
    let (_tmp, root) = setup_hooks_project();
    let server = crate::mcp_tools::build_server(root);
    let add_tool = server.get_tool("magistr_hooks_add").unwrap();

    let result = add_tool
        .invoke(serde_json::json!({
            "event": "on_enter(nonexistent)",
            "tool": "my_tool"
        }))
        .await
        .unwrap();
    let text = extract_text(&result);
    assert!(text.contains("Invalid event"));
}

#[tokio::test]
async fn hooks_remove_by_index() {
    let (_tmp, root) = setup_hooks_project();
    let server = crate::mcp_tools::build_server(root);
    let add_tool = server.get_tool("magistr_hooks_add").unwrap();
    let remove_tool = server.get_tool("magistr_hooks_remove").unwrap();

    // Add two hooks
    add_tool
        .invoke(serde_json::json!({ "event": "on_gate_pass", "tool": "tool_a" }))
        .await
        .unwrap();
    add_tool
        .invoke(serde_json::json!({ "event": "on_gate_fail", "tool": "tool_b" }))
        .await
        .unwrap();

    // Remove by index
    let result = remove_tool
        .invoke(serde_json::json!({ "index": 0 }))
        .await
        .unwrap();
    let text = extract_text(&result);
    assert!(text.contains("Removed hook"));
    assert!(text.contains("tool_a"));
    assert!(text.contains("Remaining hooks: 1"));
}

#[tokio::test]
async fn hooks_remove_by_event_and_name() {
    let (_tmp, root) = setup_hooks_project();
    let server = crate::mcp_tools::build_server(root);
    let add_tool = server.get_tool("magistr_hooks_add").unwrap();
    let remove_tool = server.get_tool("magistr_hooks_remove").unwrap();

    add_tool
        .invoke(serde_json::json!({ "event": "on_gate_pass", "tool": "tool_a" }))
        .await
        .unwrap();

    let result = remove_tool
        .invoke(serde_json::json!({ "event": "on_gate_pass", "name": "tool_a" }))
        .await
        .unwrap();
    let text = extract_text(&result);
    assert!(text.contains("Removed hook"));
    assert!(text.contains("Remaining hooks: 0"));
}

#[tokio::test]
async fn hooks_remove_not_found() {
    let (_tmp, root) = setup_hooks_project();
    let server = crate::mcp_tools::build_server(root);
    let remove_tool = server.get_tool("magistr_hooks_remove").unwrap();

    let result = remove_tool
        .invoke(serde_json::json!({ "event": "on_gate_pass", "name": "nonexistent" }))
        .await
        .unwrap();
    let text = extract_text(&result);
    assert!(text.contains("No hook found"));
}

#[tokio::test]
async fn hooks_add_remove_roundtrip_persists() {
    let (_tmp, root) = setup_hooks_project();
    let server = crate::mcp_tools::build_server(root.clone());

    // Add
    let add_tool = server.get_tool("magistr_hooks_add").unwrap();
    add_tool
        .invoke(serde_json::json!({
            "event": "on_enter(execute)",
            "tool": "scope_search",
            "arguments": {"query": "test helpers"},
            "on_failure": "warn"
        }))
        .await
        .unwrap();

    // Verify persistence by reading project.json directly
    let config: crate::types::ProjectConfig =
        serde_json::from_str(&std::fs::read_to_string(root.join(".magistr/project.json")).unwrap())
            .unwrap();
    assert_eq!(config.lifecycle_hooks.len(), 1);
    assert_eq!(config.lifecycle_hooks[0].display_name(), "scope_search");
    assert_eq!(
        config.lifecycle_hooks[0].on_failure,
        magistr_engine::OnFailure::Warn
    );

    // Remove
    let remove_tool = server.get_tool("magistr_hooks_remove").unwrap();
    remove_tool
        .invoke(serde_json::json!({ "index": 0 }))
        .await
        .unwrap();

    // Verify removal persisted
    let config: crate::types::ProjectConfig =
        serde_json::from_str(&std::fs::read_to_string(root.join(".magistr/project.json")).unwrap())
            .unwrap();
    assert!(config.lifecycle_hooks.is_empty());
}

#[tokio::test]
async fn hooks_remove_index_out_of_bounds() {
    let (_tmp, root) = setup_hooks_project();
    let server = crate::mcp_tools::build_server(root);
    let remove_tool = server.get_tool("magistr_hooks_remove").unwrap();

    let result = remove_tool
        .invoke(serde_json::json!({ "index": 99 }))
        .await
        .unwrap();
    let text = extract_text(&result);
    assert!(text.contains("out of bounds"));
}

#[tokio::test]
async fn hooks_remove_no_params_returns_error() {
    let (_tmp, root) = setup_hooks_project();
    let server = crate::mcp_tools::build_server(root);
    let remove_tool = server.get_tool("magistr_hooks_remove").unwrap();

    let result = remove_tool.invoke(serde_json::json!({})).await.unwrap();
    let text = extract_text(&result);
    assert!(text.contains("Provide either"));
}
