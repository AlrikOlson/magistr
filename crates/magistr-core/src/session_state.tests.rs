use super::*;

#[test]
fn session_state_default() {
    let state = SessionState::default();
    assert!(state.active_tool.is_none());
    assert!(!state.thinking_active);
    assert!(state.thinking_preview.is_none());
    assert!(state.session_id.is_none());
    assert!(!state.resumed);
    assert!(state.current_model.is_none());
    assert!(state.config_options.is_empty());
    assert_eq!(state.context_tokens_used, 0);
    assert_eq!(state.context_window_size, 0);
    assert!(state.completed_tools.is_empty());
}

#[test]
fn store_completed_tool_caps_at_max() {
    let mut state = SessionState::default();
    for i in 0..MAX_COMPLETED_TOOLS + 10 {
        state.store_completed_tool(CompletedTool {
            tool_use_id: format!("t{i}"),
            name: "Bash".into(),
            detail: "cmd".into(),
            input: serde_json::json!({}),
            output: None,
            is_error: false,
            kind_icon: None,
        });
    }
    assert_eq!(state.completed_tools.len(), MAX_COMPLETED_TOOLS);
    // Oldest should have been dropped — first remaining is t10.
    assert_eq!(state.completed_tools[0].tool_use_id, "t10");
}

#[test]
fn clear_thinking() {
    let mut state = SessionState::default();
    state.accumulate_thinking("step 1");
    assert!(state.thinking_active);
    assert!(state.thinking_preview.is_some());

    state.clear_thinking();
    assert!(!state.thinking_active);
    assert!(state.thinking_preview.is_none());
    assert!(state.thinking_buffer.is_empty());
}

#[test]
fn accumulate_thinking_truncates_preview() {
    let mut state = SessionState::default();
    let long_text = "x".repeat(300);
    state.accumulate_thinking(&long_text);
    assert!(state.thinking_active);
    let preview = state.thinking_preview.as_ref().unwrap();
    assert_eq!(preview.len(), 200);
}
