use super::*;

fn recs_from_json(json: serde_json::Value) -> Recommendations {
    serde_json::from_value(json).expect("valid Recommendations JSON")
}

#[test]
fn from_recommendations_maps_selections() {
    let recs = recs_from_json(serde_json::json!({
        "conventions": ["Use thiserror for library errors"],
        "tips": ["Pin toolchain in rust-toolchain.toml"],
        "reference_repos": ["https://github.com/example/repo"],
        "mcp_servers": [{"key": "context7", "name": "Context7", "description": "docs"}],
        "agents": [{"key": "cursor", "name": "Cursor", "reason": "detected IDE"}],
        "suggested_description": "A CLI tool",
        "suggested_project_type": "cli",
        "generated_files": [{"path": "CLAUDE.md", "content": "# Project"}]
    }));

    let sel = ScaffoldSelections::from_recommendations(recs);
    assert_eq!(sel.conventions.len(), 1);
    assert_eq!(sel.tips.len(), 1);
    assert_eq!(sel.reference_repos.len(), 1);
    assert_eq!(sel.selected_mcp_servers, vec!["context7"]);
    assert_eq!(sel.selected_agents, vec!["cursor"]);
    assert_eq!(sel.description, "A CLI tool");
    assert_eq!(sel.project_type, "cli");
    assert_eq!(sel.generated_files["CLAUDE.md"], "# Project");
}

#[test]
fn from_recommendations_empty_yields_defaults() {
    let sel = ScaffoldSelections::from_recommendations(Recommendations::default());
    assert!(sel.selected_agents.is_empty());
    assert!(sel.selected_mcp_servers.is_empty());
    assert!(sel.generated_files.is_empty());
    assert!(sel.lifecycle_hooks.is_none());
    assert_eq!(sel.description, "");
    assert_eq!(sel.project_type, "");
}

#[test]
fn convert_hook_requires_tool_or_command() {
    let rec: crate::scaffold::research::RecommendedLifecycleHook = serde_json::from_value(
        serde_json::json!({"event": "on_iteration_end", "description": "no action"}),
    )
    .expect("valid hook JSON");
    assert!(convert_recommended_hook(&rec).is_none());
}

#[test]
fn convert_hook_shell_command_with_on_failure() {
    let rec: crate::scaffold::research::RecommendedLifecycleHook =
        serde_json::from_value(serde_json::json!({
            "event": "on_iteration_end",
            "command": "cargo fmt",
            "description": "format after each iteration",
            "on_failure": "warn"
        }))
        .expect("valid hook JSON");
    let hook = convert_recommended_hook(&rec).expect("convertible hook");
    assert!(matches!(
        hook.action,
        magistr_engine::HookAction::ShellCommand { .. }
    ));
    assert!(matches!(hook.on_failure, magistr_engine::OnFailure::Warn));
}
