use super::*;

fn rust_gate() -> Option<QualityGate> {
    Some(QualityGate {
        check: "cargo check --all-targets".into(),
        test: "cargo test".into(),
        lint: "cargo clippy -- -D warnings".into(),
    })
}

fn node_gate() -> Option<QualityGate> {
    Some(QualityGate {
        check: "npx tsc --noEmit".into(),
        test: "npm test".into(),
        lint: "npm run lint".into(),
    })
}

fn tracker() -> WorkflowTracker {
    WorkflowTracker::new(rust_gate())
}

// ---- Phase: Researching ----

#[test]
fn starts_in_researching() {
    let t = tracker();
    assert_eq!(t.phase(), WorkflowPhase::Researching);
}

#[test]
fn mcp_tool_call_stays_in_researching() {
    let mut t = tracker();
    t.observe(&ToolEvent::McpToolCall {
        tool_name: "web_search".into(),
    });
    assert_eq!(t.phase(), WorkflowPhase::Researching);
    assert_eq!(t.research.research_tool_calls, 1);
}

#[test]
fn research_then_plan_file_transitions_to_planning() {
    let mut t = tracker();
    t.observe(&ToolEvent::McpToolCall {
        tool_name: "web_search".into(),
    });
    assert_eq!(t.phase(), WorkflowPhase::Researching);

    t.observe(&ToolEvent::FileWrite {
        path: ".magistr/current-plan.md".into(),
    });
    assert_eq!(t.phase(), WorkflowPhase::Planning);
}

#[test]
fn research_then_task_file_transitions_to_planning() {
    let mut t = tracker();
    t.observe(&ToolEvent::McpToolCall {
        tool_name: "firecrawl_scrape".into(),
    });
    t.observe(&ToolEvent::FileWrite {
        path: ".magistr/current-task.md".into(),
    });
    assert_eq!(t.phase(), WorkflowPhase::Planning);
}

#[test]
fn plan_file_alone_blocked_without_research() {
    let mut t = tracker();
    // Plan file without research → stays in Researching (research guard blocks)
    t.observe(&ToolEvent::FileWrite {
        path: ".magistr/current-plan.md".into(),
    });
    assert_eq!(t.phase(), WorkflowPhase::Researching);
}

#[test]
fn plan_file_alone_transitions_when_research_disabled() {
    let mut t = tracker();
    t.set_research_required(false);
    t.observe(&ToolEvent::FileWrite {
        path: ".magistr/current-plan.md".into(),
    });
    assert_eq!(t.phase(), WorkflowPhase::Planning);
}

#[test]
fn multiple_research_tools_accumulate() {
    let mut t = tracker();
    t.observe(&ToolEvent::McpToolCall {
        tool_name: "web_search".into(),
    });
    t.observe(&ToolEvent::McpToolCall {
        tool_name: "firecrawl_scrape".into(),
    });
    t.observe(&ToolEvent::McpToolCall {
        tool_name: "web_fetch".into(),
    });
    assert_eq!(t.research.research_tool_calls, 3);
    assert_eq!(t.phase(), WorkflowPhase::Researching);
}

#[test]
fn exploration_then_plan_transitions_to_planning() {
    let mut t = tracker();
    // Research tool call first (required by research guard)
    t.observe(&ToolEvent::McpToolCall {
        tool_name: "brave_web_search".into(),
    });
    assert_eq!(t.phase(), WorkflowPhase::Researching);

    // magistr-scope exploration (e.g. scope_modules list)
    t.observe(&ToolEvent::Exploration {
        tool_name: "magistr-scope-scope_modules".into(),
    });
    assert_eq!(t.phase(), WorkflowPhase::Researching);

    // Bash during research (e.g. cargo clippy baseline)
    t.observe(&ToolEvent::BashCommand {
        command: "cargo clippy".into(),
    });
    assert_eq!(t.phase(), WorkflowPhase::Researching);

    // Plan file written → transitions to Planning
    t.observe(&ToolEvent::FileWrite {
        path: ".magistr/current-plan.md".into(),
    });
    assert_eq!(t.phase(), WorkflowPhase::Planning);
}

#[test]
fn source_edit_during_research_skips_to_executing() {
    let mut t = tracker();
    // Research first
    t.observe(&ToolEvent::McpToolCall {
        tool_name: "brave_web_search".into(),
    });
    // Agent explores
    t.observe(&ToolEvent::Exploration {
        tool_name: "Read".into(),
    });
    // Then directly edits source (no plan file)
    t.observe(&ToolEvent::FileWrite {
        path: "src/main.rs".into(),
    });
    assert_eq!(t.phase(), WorkflowPhase::Executing);
}

#[test]
fn source_edit_without_research_stays_in_researching() {
    let mut t = tracker();
    t.observe(&ToolEvent::FileWrite {
        path: "src/main.rs".into(),
    });
    assert_eq!(t.phase(), WorkflowPhase::Researching);
}

// ---- Research nudge ----

#[test]
fn exploration_without_research_nudges_after_3_calls() {
    let mut t = tracker();
    // First 2 exploration calls — no nudge
    assert!(
        t.observe(&ToolEvent::Exploration {
            tool_name: "scope_search".into()
        })
        .is_none()
    );
    assert!(
        t.observe(&ToolEvent::Exploration {
            tool_name: "scope_read".into()
        })
        .is_none()
    );
    // 3rd exploration call — nudge fires
    let steer = t.observe(&ToolEvent::Exploration {
        tool_name: "scope_grep".into(),
    });
    assert!(
        steer.is_some(),
        "should nudge after 3 exploration calls without research"
    );
    assert!(steer.unwrap().contains("web research"));
}

#[test]
fn exploration_with_research_does_not_nudge() {
    let mut t = tracker();
    // Do research first
    t.observe(&ToolEvent::McpToolCall {
        tool_name: "brave_web_search".into(),
    });
    // Then explore — no nudge even after many calls
    for _ in 0..10 {
        assert!(
            t.observe(&ToolEvent::Exploration {
                tool_name: "scope_search".into()
            })
            .is_none()
        );
    }
}

#[test]
fn nudge_disabled_when_research_not_required() {
    let mut t = tracker();
    t.set_research_required(false);
    for _ in 0..5 {
        assert!(
            t.observe(&ToolEvent::Exploration {
                tool_name: "scope_search".into()
            })
            .is_none()
        );
    }
}

// ---- Phase: Planning ----

/// Helper to advance tracker past Research into Planning.
fn enter_planning() -> WorkflowTracker {
    let mut t = tracker();
    t.observe(&ToolEvent::McpToolCall {
        tool_name: "web_search".into(),
    });
    t.observe(&ToolEvent::FileWrite {
        path: ".magistr/current-plan.md".into(),
    });
    assert_eq!(t.phase(), WorkflowPhase::Planning);
    t
}

#[test]
fn plan_file_alone_does_not_transition() {
    let t = enter_planning();
    // Already in planning with plan file written — no transition yet
    assert!(t.plan_evidence().plan_file_written);
    assert_eq!(t.phase(), WorkflowPhase::Planning);
}

#[test]
fn source_edit_without_plan_does_not_transition() {
    let mut t = enter_planning();
    // Reset plan evidence to test this edge case
    t.plan = PlanEvidence::default();
    t.observe(&ToolEvent::FileWrite {
        path: "src/main.rs".into(),
    });
    // Source edit recorded but no plan → stays Planning
    assert_eq!(t.phase(), WorkflowPhase::Planning);
    assert_eq!(t.plan_evidence().source_edits, 1);
    assert!(!t.plan_evidence().plan_file_written);
}

#[test]
fn plan_then_source_edit_transitions_to_executing() {
    let mut t = enter_planning();

    let _steer = t.observe(&ToolEvent::FileWrite {
        path: "src/lib.rs".into(),
    });
    assert!(_steer.is_none(), "unexpected steer");
    assert_eq!(t.phase(), WorkflowPhase::Executing);
}

#[test]
fn config_file_edits_do_not_count_as_source() {
    let mut t = enter_planning();
    t.observe(&ToolEvent::FileWrite {
        path: ".claude/rules/workflow.md".into(),
    });
    assert_eq!(t.phase(), WorkflowPhase::Planning);
}

#[test]
fn task_file_tracked_separately() {
    let _t = enter_planning();
    // task_file_written already true from enter_planning's plan file,
    // but let's test fresh tracking
    let mut t2 = tracker();
    t2.observe(&ToolEvent::McpToolCall {
        tool_name: "web_search".into(),
    });
    t2.observe(&ToolEvent::FileWrite {
        path: ".magistr/current-task.md".into(),
    });
    assert!(t2.plan_evidence().task_file_written);
    assert!(!t2.plan_evidence().plan_file_written);
    assert_eq!(t2.phase(), WorkflowPhase::Planning);
}

// ---- Phase: Executing ----

fn enter_executing() -> WorkflowTracker {
    let mut t = enter_planning();
    t.observe(&ToolEvent::FileWrite {
        path: "src/lib.rs".into(),
    });
    assert_eq!(t.phase(), WorkflowPhase::Executing);
    t
}

#[test]
fn tdd_test_run_does_not_transition_to_verifying() {
    let mut t = enter_executing();

    // A bare `cargo test` during TDD should NOT trigger Verifying
    t.observe(&ToolEvent::BashCommand {
        command: "cargo test".into(),
    });
    assert_eq!(t.phase(), WorkflowPhase::Executing);
    assert!(!t.execute_evidence().quality_gate_ran);
    assert_eq!(t.execute_evidence().test_runs, 1);
}

#[test]
fn quality_gate_without_source_edits_does_not_transition() {
    // Edge case: tracker enters executing with no source files touched yet
    let mut t = enter_planning();
    t.observe(&ToolEvent::FileWrite {
        path: "src/lib.rs".into(),
    });
    // Now in Executing, but reset source count to simulate edge case
    t.execute = ExecuteEvidence::default();

    t.observe(&ToolEvent::BashCommand {
        command: "cargo check --all-targets && cargo test && cargo clippy -- -D warnings".into(),
    });
    // Gate ran but no source files touched in this phase → no transition
    assert_eq!(t.phase(), WorkflowPhase::Executing);
}

#[test]
fn full_quality_gate_with_source_edits_transitions_to_verifying() {
    let mut t = enter_executing();

    // Source file touched during execute
    t.observe(&ToolEvent::FileWrite {
        path: "src/parser.rs".into(),
    });
    assert_eq!(t.execute_evidence().source_files_touched, 1);

    // Full quality gate
    let _steer = t.observe(&ToolEvent::BashCommand {
        command: "cargo check --all-targets && cargo test && cargo clippy -- -D warnings".into(),
    });
    assert!(_steer.is_none(), "unexpected steer");
    assert_eq!(t.phase(), WorkflowPhase::Verifying);
}

#[test]
fn bare_check_command_does_not_trigger_gate() {
    let mut t = enter_executing();
    t.observe(&ToolEvent::FileWrite {
        path: "src/foo.rs".into(),
    });
    t.observe(&ToolEvent::BashCommand {
        command: "cargo check --all-targets".into(),
    });
    assert_eq!(t.phase(), WorkflowPhase::Executing);
    assert!(!t.execute_evidence().quality_gate_ran);
}

#[test]
fn node_project_npm_test_alone_does_not_trigger_gate() {
    let mut t = WorkflowTracker::new(node_gate());
    // Research phase
    t.observe(&ToolEvent::McpToolCall {
        tool_name: "web_search".into(),
    });
    t.observe(&ToolEvent::FileWrite {
        path: ".magistr/current-plan.md".into(),
    });
    assert_eq!(t.phase(), WorkflowPhase::Planning);
    t.observe(&ToolEvent::FileWrite {
        path: "src/index.ts".into(),
    });
    assert_eq!(t.phase(), WorkflowPhase::Executing);

    t.observe(&ToolEvent::FileWrite {
        path: "src/auth.ts".into(),
    });
    // Just `npm test` (TDD) should NOT trigger gate — only lint does
    t.observe(&ToolEvent::BashCommand {
        command: "npm test".into(),
    });
    assert_eq!(t.phase(), WorkflowPhase::Executing);
    assert!(!t.execute_evidence().quality_gate_ran);
}

// ---- Phase: Verifying ----

fn enter_verifying() -> WorkflowTracker {
    let mut t = enter_executing();
    t.observe(&ToolEvent::FileWrite {
        path: "src/parser.rs".into(),
    });
    t.observe(&ToolEvent::BashCommand {
        command: "cargo check --all-targets && cargo test && cargo clippy -- -D warnings".into(),
    });
    assert_eq!(t.phase(), WorkflowPhase::Verifying);
    t
}

#[test]
fn spec_update_alone_does_not_complete() {
    let mut t = enter_verifying();
    t.observe(&ToolEvent::FileWrite {
        path: "ROADMAP.md".into(),
    });
    // Spec updated but no quality gate ran in verify phase → incomplete
    assert_eq!(t.phase(), WorkflowPhase::Verifying);
    assert!(t.verify_evidence().spec_updated);
}

#[test]
fn git_commit_alone_does_not_complete() {
    let mut t = enter_verifying();
    t.observe(&ToolEvent::GitCommit);
    assert_eq!(t.phase(), WorkflowPhase::Verifying);
    assert!(t.verify_evidence().committed);
}

#[test]
fn quality_gate_plus_spec_update_completes() {
    let mut t = enter_verifying();
    t.observe(&ToolEvent::BashCommand {
        command: "cargo check --all-targets && cargo test && cargo clippy -- -D warnings".into(),
    });
    assert_eq!(t.phase(), WorkflowPhase::Verifying);

    let _steer = t.observe(&ToolEvent::FileWrite {
        path: "ROADMAP.md".into(),
    });
    assert!(_steer.is_none(), "unexpected steer");
    assert_eq!(t.phase(), WorkflowPhase::Complete);
}

#[test]
fn quality_gate_plus_git_commit_completes() {
    let mut t = enter_verifying();
    t.observe(&ToolEvent::BashCommand {
        command: "cargo test && cargo clippy -- -D warnings".into(),
    });
    let _steer = t.observe(&ToolEvent::GitCommit);
    assert!(_steer.is_none(), "unexpected steer");
    assert_eq!(t.phase(), WorkflowPhase::Complete);
}

#[test]
fn quality_gate_result_tracked() {
    let mut t = enter_verifying();
    t.observe(&ToolEvent::BashCommand {
        command: "cargo test && cargo clippy -- -D warnings".into(),
    });
    t.observe(&ToolEvent::BashResult {
        command: "cargo test && cargo clippy -- -D warnings".into(),
        is_error: false,
    });
    assert!(t.verify_evidence().quality_gate_passed);
}

#[test]
fn failed_quality_gate_result_tracked() {
    let mut t = enter_verifying();
    t.observe(&ToolEvent::BashCommand {
        command: "cargo test".into(),
    });
    t.observe(&ToolEvent::BashResult {
        command: "cargo test".into(),
        is_error: true,
    });
    assert!(!t.verify_evidence().quality_gate_passed);
}

// ---- Phase: Complete (no further transitions) ----

#[test]
fn complete_phase_ignores_events() {
    let mut t = enter_verifying();
    t.observe(&ToolEvent::BashCommand {
        command: "cargo test && cargo clippy -- -D warnings".into(),
    });
    t.observe(&ToolEvent::FileWrite {
        path: "ROADMAP.md".into(),
    });
    assert_eq!(t.phase(), WorkflowPhase::Complete);

    // Further events should not change anything
    let _steer = t.observe(&ToolEvent::FileWrite {
        path: "src/new_file.rs".into(),
    });
    assert!(_steer.is_none(), "unexpected steer");
    assert_eq!(t.phase(), WorkflowPhase::Complete);
}

// ---- Full lifecycle test ----

#[test]
fn full_plan_execute_verify_lifecycle() {
    let mut t = tracker();

    // RESEARCH: Web search
    t.observe(&ToolEvent::McpToolCall {
        tool_name: "web_search".into(),
    });
    t.observe(&ToolEvent::McpToolCall {
        tool_name: "firecrawl_scrape".into(),
    });
    assert_eq!(t.phase(), WorkflowPhase::Researching);

    // PLAN: Orient, write plan
    t.observe(&ToolEvent::BashCommand {
        command: "cargo check --all-targets".into(),
    });
    // Still researching — no plan file yet
    assert_eq!(t.phase(), WorkflowPhase::Researching);
    t.observe(&ToolEvent::FileWrite {
        path: ".magistr/current-plan.md".into(),
    });
    assert_eq!(t.phase(), WorkflowPhase::Planning);
    t.observe(&ToolEvent::FileWrite {
        path: ".magistr/current-task.md".into(),
    });
    assert_eq!(t.phase(), WorkflowPhase::Planning);
    assert!(t.plan_evidence().plan_file_written);
    assert!(t.plan_evidence().task_file_written);

    // EXECUTE: TDD cycle
    t.observe(&ToolEvent::FileWrite {
        path: "tests/parser_test.rs".into(),
    });
    assert_eq!(t.phase(), WorkflowPhase::Executing); // plan + source edit

    t.observe(&ToolEvent::BashCommand {
        command: "cargo test".into(),
    }); // TDD red
    assert_eq!(t.phase(), WorkflowPhase::Executing);

    t.observe(&ToolEvent::FileWrite {
        path: "src/parser.rs".into(),
    });
    t.observe(&ToolEvent::BashCommand {
        command: "cargo test".into(),
    }); // TDD green
    assert_eq!(t.phase(), WorkflowPhase::Executing);

    // VERIFY: Full quality gate
    t.observe(&ToolEvent::BashCommand {
        command: "cargo check --all-targets && cargo test && cargo clippy -- -D warnings".into(),
    });
    assert_eq!(t.phase(), WorkflowPhase::Verifying);

    t.observe(&ToolEvent::BashResult {
        command: "cargo check --all-targets && cargo test && cargo clippy -- -D warnings".into(),
        is_error: false,
    });
    assert!(t.verify_evidence().quality_gate_passed);

    // Update spec and commit
    t.observe(&ToolEvent::FileWrite {
        path: "ROADMAP.md".into(),
    });
    assert_eq!(t.phase(), WorkflowPhase::Complete);
}

// ---- Reset ----

#[test]
fn reset_returns_to_researching() {
    let mut t = enter_verifying();
    t.observe(&ToolEvent::BashCommand {
        command: "cargo test && cargo clippy -- -D warnings".into(),
    });
    t.observe(&ToolEvent::GitCommit);
    assert_eq!(t.phase(), WorkflowPhase::Complete);

    t.reset();
    assert_eq!(t.phase(), WorkflowPhase::Researching);
    assert_eq!(t.research.research_tool_calls, 0);
    assert!(!t.plan_evidence().plan_file_written);
    assert_eq!(t.execute_evidence().source_files_touched, 0);
    assert!(!t.verify_evidence().committed);
}

// ---- TodoWrite tracking ----

#[test]
fn todo_write_updates_items() {
    let mut t = tracker();
    t.observe(&ToolEvent::TodoWrite {
        todos: vec![
            TodoItem {
                id: "1".into(),
                content: "Set up boilerplate".into(),
                status: TodoStatus::Completed,
            },
            TodoItem {
                id: "2".into(),
                content: "Implement parser".into(),
                status: TodoStatus::InProgress,
            },
            TodoItem {
                id: "3".into(),
                content: "Add tests".into(),
                status: TodoStatus::Pending,
            },
        ],
    });
    assert_eq!(t.todos().len(), 3);
    assert_eq!(t.todos()[1].status, TodoStatus::InProgress);
}

#[test]
fn todo_write_does_not_trigger_phase_transition() {
    let mut t = tracker();
    let _steer = t.observe(&ToolEvent::TodoWrite {
        todos: vec![TodoItem {
            id: "1".into(),
            content: "task".into(),
            status: TodoStatus::Pending,
        }],
    });
    assert!(_steer.is_none(), "unexpected steer");
    assert_eq!(t.phase(), WorkflowPhase::Researching);
}

#[test]
fn todo_write_replaces_previous_list() {
    let mut t = tracker();
    t.observe(&ToolEvent::TodoWrite {
        todos: vec![TodoItem {
            id: "1".into(),
            content: "first".into(),
            status: TodoStatus::Pending,
        }],
    });
    assert_eq!(t.todos().len(), 1);

    t.observe(&ToolEvent::TodoWrite {
        todos: vec![
            TodoItem {
                id: "1".into(),
                content: "first".into(),
                status: TodoStatus::Completed,
            },
            TodoItem {
                id: "2".into(),
                content: "second".into(),
                status: TodoStatus::InProgress,
            },
        ],
    });
    assert_eq!(t.todos().len(), 2);
    assert_eq!(t.todos()[0].status, TodoStatus::Completed);
}

// ---- parse_todo_write ----

#[test]
fn parse_todo_write_valid() {
    let input = serde_json::json!({
        "todos": [
            {"id": "1", "content": "Setup", "status": "completed"},
            {"id": "2", "content": "Build", "status": "in_progress"},
            {"id": "3", "content": "Test", "status": "pending"},
        ]
    });
    let items = parse_todo_write(&input);
    assert_eq!(items.len(), 3);
    assert_eq!(items[0].status, TodoStatus::Completed);
    assert_eq!(items[1].status, TodoStatus::InProgress);
    assert_eq!(items[2].status, TodoStatus::Pending);
}

#[test]
fn parse_todo_write_empty_array() {
    let input = serde_json::json!({"todos": []});
    assert!(parse_todo_write(&input).is_empty());
}

#[test]
fn parse_todo_write_missing_field() {
    let input = serde_json::json!({"something_else": true});
    assert!(parse_todo_write(&input).is_empty());
}

#[test]
fn parse_todo_write_malformed_items() {
    let input = serde_json::json!({
        "todos": [
            {"id": "1"},  // missing content
            {"id": "2", "content": "Valid", "status": "pending"},
        ]
    });
    let items = parse_todo_write(&input);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, "2");
}

// ---- tool_event_from ----

#[test]
fn tool_event_from_write() {
    let event = tool_event_from(
        None,
        "Write",
        &serde_json::json!({"file_path": "src/main.rs", "content": "fn main() {}"}),
        &[],
    );
    assert!(matches!(event, Some(ToolEvent::FileWrite { path }) if path == "src/main.rs"));
}

#[test]
fn tool_event_from_edit() {
    let event = tool_event_from(
        None,
        "Edit",
        &serde_json::json!({"file_path": "src/lib.rs", "old_string": "a", "new_string": "b"}),
        &[],
    );
    assert!(matches!(event, Some(ToolEvent::FileWrite { path }) if path == "src/lib.rs"));
}

#[test]
fn tool_event_from_bash() {
    let event = tool_event_from(
        None,
        "Bash",
        &serde_json::json!({"command": "cargo test"}),
        &[],
    );
    assert!(matches!(event, Some(ToolEvent::BashCommand { command }) if command == "cargo test"));
}

#[test]
fn tool_event_from_bash_git_commit() {
    let event = tool_event_from(
        None,
        "Bash",
        &serde_json::json!({"command": "git commit -m 'feat: add parser'"}),
        &[],
    );
    assert!(matches!(event, Some(ToolEvent::GitCommit)));
}

#[test]
fn tool_event_from_todo_write() {
    let event = tool_event_from(
        None,
        "TodoWrite",
        &serde_json::json!({"todos": [{"id": "1", "content": "task", "status": "pending"}]}),
        &[],
    );
    assert!(matches!(event, Some(ToolEvent::TodoWrite { todos }) if todos.len() == 1));
}

#[test]
fn tool_event_from_read_is_exploration() {
    let event = tool_event_from(
        None,
        "Read",
        &serde_json::json!({"file_path": "src/main.rs"}),
        &[],
    );
    assert!(matches!(event, Some(ToolEvent::Exploration { tool_name }) if tool_name == "Read"));
}

#[test]
fn tool_event_from_unknown_tool() {
    let event = tool_event_from(
        None,
        "SomeCompletelyUnknownTool",
        &serde_json::json!({"random": "data"}),
        &[],
    );
    assert!(event.is_none());
}

#[test]
fn tool_event_from_magistr_scope_is_exploration() {
    let event = tool_event_from(
        None,
        "magistr-scope-scope_modules",
        &serde_json::json!({"action": "list"}),
        &[],
    );
    // cs_modules input has "action" key but isn't TodoWrite
    // With name-based fallback, it matches is_exploration_tool
    assert!(matches!(
        event,
        Some(ToolEvent::TodoWrite { .. }) | Some(ToolEvent::Exploration { .. })
    ));
}

#[test]
fn tool_event_from_web_search() {
    let event = tool_event_from(
        None,
        "web_search",
        &serde_json::json!({"query": "ratatui best practices"}),
        &[],
    );
    assert!(
        matches!(event, Some(ToolEvent::McpToolCall { tool_name }) if tool_name == "web_search")
    );
}

#[test]
fn tool_event_from_firecrawl() {
    let event = tool_event_from(
        None,
        "firecrawl_scrape",
        &serde_json::json!({"url": "https://docs.rs"}),
        &[],
    );
    assert!(
        matches!(event, Some(ToolEvent::McpToolCall { tool_name }) if tool_name == "firecrawl_scrape")
    );
}

#[test]
fn tool_event_from_mcp_prefixed_research_tool() {
    let event = tool_event_from(None, "mcp__firecrawl__scrape", &serde_json::json!({}), &[]);
    assert!(matches!(event, Some(ToolEvent::McpToolCall { .. })));
}

#[test]
fn tool_event_from_web_fetch() {
    let event = tool_event_from(
        None,
        "web_fetch",
        &serde_json::json!({"url": "https://example.com"}),
        &[],
    );
    assert!(matches!(event, Some(ToolEvent::McpToolCall { .. })));
}

#[test]
fn tool_event_from_edit_with_path_field() {
    // ACP backends use "path" instead of "file_path"
    let event = tool_event_from(
        Some(crate::ToolKind::Edit),
        "Editing src/main.rs",
        &serde_json::json!({"path": "src/main.rs", "new_string": "fn main() {}"}),
        &[],
    );
    assert!(matches!(event, Some(ToolEvent::FileWrite { path }) if path == "src/main.rs"));
}

#[test]
fn tool_event_from_edit_with_location_fallback() {
    // When input has no path field, fall back to ACP locations
    let locations = vec![crate::ToolLocation {
        path: "src/lib.rs".to_string(),
        line: Some(42),
    }];
    let event = tool_event_from(
        Some(crate::ToolKind::Edit),
        "Editing src/lib.rs",
        &serde_json::json!({"new_string": "pub fn foo() {}"}),
        &locations,
    );
    assert!(matches!(event, Some(ToolEvent::FileWrite { path }) if path == "src/lib.rs"));
}

#[test]
fn tool_event_from_write_plan_detected() {
    // Verify plan file writes are correctly identified (triggers phase transition)
    let event = tool_event_from(
        Some(crate::ToolKind::Edit),
        "Creating .magistr/current-plan.md",
        &serde_json::json!({"path": ".magistr/current-plan.md", "content": "# Plan\n..."}),
        &[],
    );
    assert!(
        matches!(event, Some(ToolEvent::FileWrite { ref path }) if path.contains("current-plan"))
    );

    // Feed it to a tracker and verify phase advances (with research evidence first)
    let mut tracker = WorkflowTracker::new(None);
    assert_eq!(tracker.phase(), WorkflowPhase::Researching);
    tracker.observe(&ToolEvent::McpToolCall {
        tool_name: "brave_web_search".into(),
    });
    tracker.observe(&event.unwrap());
    assert_eq!(tracker.phase(), WorkflowPhase::Planning);
}

#[test]
fn is_research_tool_matches() {
    use crate::tools::patterns::ToolPatterns;

    // Built-in / generic
    assert!(ToolPatterns::is_research("web_search"));
    assert!(ToolPatterns::is_research("WebSearch"));
    assert!(ToolPatterns::is_research("web_fetch"));
    // Firecrawl MCP
    assert!(ToolPatterns::is_research("firecrawl_scrape"));
    assert!(ToolPatterns::is_research("firecrawl_search"));
    assert!(ToolPatterns::is_research("mcp__firecrawl__scrape"));
    // Brave Search MCP
    assert!(ToolPatterns::is_research("brave_web_search"));
    assert!(ToolPatterns::is_research("brave_news_search"));
    assert!(ToolPatterns::is_research("brave_summarizer"));
    assert!(ToolPatterns::is_research(
        "mcp__brave-search__brave_web_search"
    ));
    // Context7 MCP
    assert!(ToolPatterns::is_research("context7"));
    assert!(ToolPatterns::is_research(
        "mcp__context7__resolve_library_id"
    ));
    // Perplexity / Tavily
    assert!(ToolPatterns::is_research("mcp__tavily__search"));
    assert!(ToolPatterns::is_research("mcp__perplexity__ask"));
    // Non-research tools — codebase exploration is NOT research
    assert!(!ToolPatterns::is_research("Read"));
    assert!(!ToolPatterns::is_research("Write"));
    assert!(!ToolPatterns::is_research("Bash"));
    assert!(!ToolPatterns::is_research("scope_search"));
    assert!(!ToolPatterns::is_research("mcp__jig-scope__scope_search"));
    assert!(!ToolPatterns::is_research("mcp__jig-scope__scope_grep"));
}

// ---- Path classification ----

#[test]
fn path_classification() {
    assert!(is_config_path(".claude/rules/workflow.md"));
    assert!(is_config_path("/project/.claude/project.json"));
    assert!(is_config_path(".magistr/project.json"));
    assert!(is_config_path("/project/.magistr/project.json"));
    assert!(!is_config_path("src/main.rs"));

    assert!(is_spec_file("ROADMAP.md"));
    assert!(is_spec_file("roadmap.md"));
    assert!(!is_spec_file("src/main.rs"));

    assert!(is_test_file("tests/parser_test.rs"));
    assert!(is_test_file("src/parser.spec.ts"));
    assert!(!is_test_file("src/main.rs"));

    assert!(is_plan_file(".magistr/current-plan.md"));
    assert!(is_plan_file(".claude/current-plan.md")); // backward compat
    assert!(!is_plan_file(".claude/rules/workflow.md"));
}

// ---- Quality gate command matching ----

#[test]
fn quality_gate_with_config() {
    let gate = rust_gate();
    // Full gate with test + lint → matches
    assert!(is_quality_gate_command(
        "cargo check --all-targets && cargo test && cargo clippy -- -D warnings",
        &gate
    ));
    // Just lint → matches (lint alone is a gate signal)
    assert!(is_quality_gate_command(
        "cargo clippy -- -D warnings",
        &gate
    ));
    // Just test → does NOT match (could be TDD)
    assert!(!is_quality_gate_command("cargo test", &gate));
    // Bare check command → does NOT match
    assert!(!is_quality_gate_command("cargo check --all-targets", &gate));
    assert!(!is_quality_gate_command("echo hello", &gate));
}

#[test]
fn quality_gate_fallback() {
    let gate = None;
    // Lint alone → matches
    assert!(is_quality_gate_command(
        "cargo clippy -- -D warnings",
        &gate
    ));
    // Test alone → does NOT match
    assert!(!is_quality_gate_command("cargo test", &gate));
    // Check alone → does NOT match
    assert!(!is_quality_gate_command("cargo check --all-targets", &gate));
}

#[test]
fn test_only_command_detection() {
    assert!(is_test_only_command("cargo test"));
    assert!(is_test_only_command("npm test"));
    assert!(!is_test_only_command(
        "cargo test && cargo clippy -- -D warnings"
    ));
    assert!(!is_test_only_command("echo hello"));
}

// ── Multi-stack gate detection ──────────────────────────────

#[test]
fn gate_detection_rust() {
    let gate = rust_gate();
    assert!(is_quality_gate_command(
        "cargo check --all-targets && cargo test && cargo clippy -- -D warnings",
        &gate
    ));
    assert!(is_quality_gate_command(
        "cargo clippy -- -D warnings",
        &gate
    ));
    assert!(!is_quality_gate_command("cargo test", &gate));
    assert!(is_test_only_command("cargo test"));
    assert!(is_test_only_command("cargo test --workspace"));
    assert!(!is_test_only_command(
        "cargo test && cargo clippy -- -D warnings"
    ));
}

#[test]
fn gate_detection_node() {
    let gate = node_gate();
    assert!(is_quality_gate_command("npm run lint && npm test", &gate));
    assert!(is_quality_gate_command("npm run lint", &gate));
    assert!(!is_quality_gate_command("npm test", &gate));
    assert!(is_test_only_command("npm test"));
    assert!(is_test_only_command("npx jest"));
    assert!(!is_test_only_command("npm run lint && npm test"));
}

fn python_gate() -> Option<QualityGate> {
    Some(QualityGate {
        check: "python -m mypy .".into(),
        test: "pytest".into(),
        lint: "ruff check .".into(),
    })
}

#[test]
fn gate_detection_python() {
    let gate = python_gate();
    assert!(is_quality_gate_command(
        "python -m mypy . && pytest && ruff check .",
        &gate
    ));
    assert!(is_quality_gate_command("ruff check .", &gate));
    assert!(!is_quality_gate_command("pytest", &gate));
    assert!(is_test_only_command("pytest"));
    assert!(is_test_only_command("python -m pytest tests/"));
    assert!(!is_test_only_command("pytest && ruff check ."));
}

fn go_gate() -> Option<QualityGate> {
    Some(QualityGate {
        check: "go build ./...".into(),
        test: "go test ./...".into(),
        lint: "golangci-lint run".into(),
    })
}

#[test]
fn gate_detection_go() {
    let gate = go_gate();
    assert!(is_quality_gate_command(
        "go build ./... && go test ./... && golangci-lint run",
        &gate
    ));
    assert!(is_quality_gate_command("golangci-lint run", &gate));
    assert!(!is_quality_gate_command("go test ./...", &gate));
    assert!(is_test_only_command("go test ./..."));
    assert!(!is_test_only_command("go test ./... && golangci-lint run"));
}

fn java_gate() -> Option<QualityGate> {
    Some(QualityGate {
        check: "mvn compile".into(),
        test: "mvn test".into(),
        lint: "mvn verify".into(),
    })
}

#[test]
fn gate_detection_java() {
    let gate = java_gate();
    assert!(is_quality_gate_command("mvn verify", &gate));
    assert!(!is_quality_gate_command("mvn test", &gate));
}

// ---- Edge cases / regression guards ----

#[test]
fn editing_eslint_config_during_plan_does_not_transition() {
    let mut t = enter_planning();
    t.observe(&ToolEvent::FileWrite {
        path: ".claude/tsconfig.eslint.json".into(),
    });
    assert_eq!(t.phase(), WorkflowPhase::Planning);
}

#[test]
fn running_tsc_no_emit_during_execute_does_not_trigger_verify() {
    let mut t = enter_executing();
    t.observe(&ToolEvent::FileWrite {
        path: "src/index.ts".into(),
    });
    t.observe(&ToolEvent::BashCommand {
        command: "npx tsc --noEmit 2>&1".into(),
    });
    assert_eq!(t.phase(), WorkflowPhase::Executing);
}

#[test]
fn multiple_tdd_cycles_stay_in_execute() {
    let mut t = enter_executing();

    for i in 0..5 {
        t.observe(&ToolEvent::FileWrite {
            path: format!("tests/test_{i}.rs"),
        });
        t.observe(&ToolEvent::BashCommand {
            command: "cargo test".into(),
        });
        t.observe(&ToolEvent::FileWrite {
            path: format!("src/module_{i}.rs"),
        });
        t.observe(&ToolEvent::BashCommand {
            command: "cargo test".into(),
        });
    }

    // Still executing after 5 TDD cycles (no full gate ran)
    assert_eq!(t.phase(), WorkflowPhase::Executing);
    assert_eq!(t.execute_evidence().test_runs, 10);
    assert_eq!(t.execute_evidence().source_files_touched, 5);
    assert_eq!(t.execute_evidence().test_files_touched, 5);
}

#[test]
fn phase_labels_and_step_indices() {
    assert_eq!(WorkflowPhase::Researching.label(), "Researching");
    assert_eq!(WorkflowPhase::Planning.label(), "Planning");
    assert_eq!(WorkflowPhase::Executing.label(), "Executing");
    assert_eq!(WorkflowPhase::Verifying.label(), "Verifying");
    assert_eq!(WorkflowPhase::Complete.label(), "Complete");

    assert_eq!(WorkflowPhase::Researching.step_index(), 0);
    assert_eq!(WorkflowPhase::Planning.step_index(), 1);
    assert_eq!(WorkflowPhase::Executing.step_index(), 2);
    assert_eq!(WorkflowPhase::Verifying.step_index(), 3);
    assert_eq!(WorkflowPhase::Complete.step_index(), 4);
}

/// Verify that Complete can fire before the git commit happens.
///
/// This is by design: the tracker transitions to Complete when it sees
/// ROADMAP updated + quality gate ran (in the Verify phase). The git
/// commit typically follows *after* that. Callers (especially the TUI
/// loop) must NOT treat Complete as a signal to abort the agent session
/// — the agent still needs to commit, clean up plan files, etc.
/// Complete is a progress-bar indicator, not a session-termination signal.
#[test]
fn complete_fires_before_commit() {
    let mut t = enter_verifying();

    // Run quality gate in the Verify phase
    t.observe(&ToolEvent::BashCommand {
        command: "cargo check --all-targets && cargo test && cargo clippy -- -D warnings".into(),
    });
    assert_eq!(t.phase(), WorkflowPhase::Verifying);

    // Update spec — this triggers Complete (gate + closure)
    t.observe(&ToolEvent::FileWrite {
        path: "ROADMAP.md".into(),
    });
    assert_eq!(t.phase(), WorkflowPhase::Complete);

    // The commit hasn't happened yet — this is the key assertion.
    // The TUI must NOT abort the agent session here; the agent still
    // needs to run `git commit`, clean up `.magistr/current-plan.md`, etc.
    assert!(
        !t.verify_evidence().committed,
        "Complete must fire before the commit — the agent still needs to commit"
    );
}

// ---- ACP plan-driven phase classification ----

#[test]
fn classify_empty_entries_returns_none() {
    assert!(classify_plan_phase(&[]).is_none());
}

#[test]
fn classify_all_completed_returns_complete() {
    use crate::{AcpPlanEntry, PlanEntryPriority, PlanEntryStatus};
    let entries = vec![
        AcpPlanEntry {
            content: "[Plan] Read codebase".into(),
            priority: PlanEntryPriority::High,
            status: PlanEntryStatus::Completed,
        },
        AcpPlanEntry {
            content: "[Execute] Build feature".into(),
            priority: PlanEntryPriority::Medium,
            status: PlanEntryStatus::Completed,
        },
        AcpPlanEntry {
            content: "[Verify] Commit".into(),
            priority: PlanEntryPriority::Medium,
            status: PlanEntryStatus::Completed,
        },
    ];
    assert_eq!(classify_plan_phase(&entries), Some(WorkflowPhase::Complete));
}

#[test]
fn classify_first_in_progress_determines_phase() {
    use crate::{AcpPlanEntry, PlanEntryPriority, PlanEntryStatus};
    let entries = vec![
        AcpPlanEntry {
            content: "[Plan] Read spec".into(),
            priority: PlanEntryPriority::High,
            status: PlanEntryStatus::Completed,
        },
        AcpPlanEntry {
            content: "[Execute] Write tests".into(),
            priority: PlanEntryPriority::Medium,
            status: PlanEntryStatus::InProgress,
        },
        AcpPlanEntry {
            content: "[Verify] Run gate".into(),
            priority: PlanEntryPriority::Medium,
            status: PlanEntryStatus::Pending,
        },
    ];
    assert_eq!(
        classify_plan_phase(&entries),
        Some(WorkflowPhase::Executing)
    );
}

#[test]
fn classify_all_pending_returns_researching() {
    use crate::{AcpPlanEntry, PlanEntryPriority, PlanEntryStatus};
    let entries = vec![
        AcpPlanEntry {
            content: "[Execute] Build it".into(),
            priority: PlanEntryPriority::Medium,
            status: PlanEntryStatus::Pending,
        },
        AcpPlanEntry {
            content: "[Verify] Check it".into(),
            priority: PlanEntryPriority::Medium,
            status: PlanEntryStatus::Pending,
        },
    ];
    assert_eq!(
        classify_plan_phase(&entries),
        Some(WorkflowPhase::Researching)
    );
}

#[test]
fn classify_no_in_progress_uses_last_completed() {
    use crate::{AcpPlanEntry, PlanEntryPriority, PlanEntryStatus};
    // Between phases: plan entries done, execute not started yet
    let entries = vec![
        AcpPlanEntry {
            content: "[Plan] Read codebase".into(),
            priority: PlanEntryPriority::High,
            status: PlanEntryStatus::Completed,
        },
        AcpPlanEntry {
            content: "[Execute] Write code".into(),
            priority: PlanEntryPriority::Medium,
            status: PlanEntryStatus::Pending,
        },
    ];
    // Last completed is a [Plan] entry → stays Planning
    assert_eq!(classify_plan_phase(&entries), Some(WorkflowPhase::Planning));
}

#[test]
fn classify_entry_phase_explicit_tags() {
    assert_eq!(
        classify_entry_phase("[Research] Web search for best practices"),
        WorkflowPhase::Researching
    );
    assert_eq!(
        classify_entry_phase("[Research] Scrape docs for ratatui API"),
        WorkflowPhase::Researching
    );
    assert_eq!(
        classify_entry_phase("[Plan] Read spec"),
        WorkflowPhase::Planning
    );
    assert_eq!(
        classify_entry_phase("[Planning] Orient"),
        WorkflowPhase::Planning
    );
    assert_eq!(
        classify_entry_phase("[Execute] Write tests"),
        WorkflowPhase::Executing
    );
    assert_eq!(
        classify_entry_phase("[Executing] Build module"),
        WorkflowPhase::Executing
    );
    assert_eq!(
        classify_entry_phase("[Implement] Add feature"),
        WorkflowPhase::Executing
    );
    assert_eq!(
        classify_entry_phase("[Verify] Run quality gate"),
        WorkflowPhase::Verifying
    );
    assert_eq!(
        classify_entry_phase("[Verifying] Update spec"),
        WorkflowPhase::Verifying
    );
    assert_eq!(
        classify_entry_phase("[Close] Clean up"),
        WorkflowPhase::Verifying
    );
}

#[test]
fn classify_entry_phase_keyword_fallback() {
    // Research keywords
    assert_eq!(
        classify_entry_phase("Web search for ratatui patterns"),
        WorkflowPhase::Researching
    );
    assert_eq!(
        classify_entry_phase("Scrape docs for crossterm API"),
        WorkflowPhase::Researching
    );
    assert_eq!(
        classify_entry_phase("Use firecrawl to get changelog"),
        WorkflowPhase::Researching
    );

    // Execute keywords
    assert_eq!(
        classify_entry_phase("Implement auth module"),
        WorkflowPhase::Executing
    );
    assert_eq!(
        classify_entry_phase("Write tests for parser"),
        WorkflowPhase::Executing
    );
    assert_eq!(
        classify_entry_phase("Create test file"),
        WorkflowPhase::Executing
    );
    assert_eq!(
        classify_entry_phase("TDD red-green-refactor"),
        WorkflowPhase::Executing
    );

    // Verify keywords
    assert_eq!(
        classify_entry_phase("Run quality gate"),
        WorkflowPhase::Verifying
    );
    assert_eq!(
        classify_entry_phase("Update ROADMAP and commit"),
        WorkflowPhase::Verifying
    );
    assert_eq!(
        classify_entry_phase("Git commit changes"),
        WorkflowPhase::Verifying
    );
    assert_eq!(
        classify_entry_phase("Clean up state files"),
        WorkflowPhase::Verifying
    );

    // Default to Planning
    assert_eq!(
        classify_entry_phase("Read the codebase"),
        WorkflowPhase::Planning
    );
    assert_eq!(
        classify_entry_phase("Explore architecture"),
        WorkflowPhase::Planning
    );
    assert_eq!(
        classify_entry_phase("Analyze dependencies"),
        WorkflowPhase::Planning
    );
}

// ---- PlanUpdate event tests ----

#[test]
fn plan_update_advances_phase_forward() {
    let mut t = tracker();
    assert_eq!(t.phase(), WorkflowPhase::Researching);

    // PlanUpdate with Executing classified phase should jump forward
    let _steer = t.observe(&ToolEvent::PlanUpdate {
        classified_phase: Some(WorkflowPhase::Executing),
        all_complete: false,
    });
    assert!(_steer.is_none(), "unexpected steer");
    assert_eq!(t.phase(), WorkflowPhase::Executing);
}

#[test]
fn plan_update_never_regresses_phase() {
    let mut t = tracker();
    // Advance to Executing
    t.observe(&ToolEvent::PlanUpdate {
        classified_phase: Some(WorkflowPhase::Executing),
        all_complete: false,
    });
    assert_eq!(t.phase(), WorkflowPhase::Executing);

    // PlanUpdate with Planning should NOT regress
    let _steer = t.observe(&ToolEvent::PlanUpdate {
        classified_phase: Some(WorkflowPhase::Planning),
        all_complete: false,
    });
    assert!(_steer.is_none(), "unexpected steer");
    assert_eq!(t.phase(), WorkflowPhase::Executing);
}

#[test]
fn plan_update_all_complete_transitions_to_complete() {
    let mut t = tracker();
    let _steer = t.observe(&ToolEvent::PlanUpdate {
        classified_phase: None,
        all_complete: true,
    });
    assert!(_steer.is_none(), "unexpected steer");
    assert_eq!(t.phase(), WorkflowPhase::Complete);
}

#[test]
fn plan_update_none_phase_no_change() {
    let mut t = tracker();
    let _steer = t.observe(&ToolEvent::PlanUpdate {
        classified_phase: None,
        all_complete: false,
    });
    assert!(_steer.is_none(), "unexpected steer");
    assert_eq!(t.phase(), WorkflowPhase::Researching);
}

#[test]
fn plan_update_and_tool_events_cooperate() {
    let mut t = WorkflowTracker::new(rust_gate());

    // PlanUpdate advances to Executing
    t.observe(&ToolEvent::PlanUpdate {
        classified_phase: Some(WorkflowPhase::Executing),
        all_complete: false,
    });
    assert_eq!(t.phase(), WorkflowPhase::Executing);

    // Tool events accumulate evidence: source file write + quality gate
    t.observe(&ToolEvent::FileWrite {
        path: "src/main.rs".into(),
    });
    let _steer = t.observe(&ToolEvent::BashCommand {
        command: "cargo check --all-targets && cargo test && cargo clippy -- -D warnings".into(),
    });
    assert!(_steer.is_none(), "unexpected steer");
    assert_eq!(t.phase(), WorkflowPhase::Verifying);
}

#[test]
fn broader_execute_keywords_match() {
    assert_eq!(
        classify_entry_phase("Start implementing the auth module"),
        WorkflowPhase::Executing
    );
    assert_eq!(
        classify_entry_phase("Set up the database schema"),
        WorkflowPhase::Executing
    );
    assert_eq!(
        classify_entry_phase("Create struct for user model"),
        WorkflowPhase::Executing
    );
    assert_eq!(
        classify_entry_phase("Add module for error handling"),
        WorkflowPhase::Executing
    );
    assert_eq!(
        classify_entry_phase("Scaffold the project structure"),
        WorkflowPhase::Executing
    );
}

#[test]
fn extract_path_from_title_works() {
    assert_eq!(
        extract_path_from_title("Editing src/main.rs"),
        Some("src/main.rs".to_string())
    );
    assert_eq!(
        extract_path_from_title("Creating .magistr/current-plan.md"),
        Some(".magistr/current-plan.md".to_string())
    );
    assert_eq!(
        extract_path_from_title("Reading config.toml"),
        Some("config.toml".to_string())
    );
    assert_eq!(
        extract_path_from_title("Deleting /Users/foo/bar.txt"),
        Some("/Users/foo/bar.txt".to_string())
    );
    // No verb prefix → None
    assert_eq!(extract_path_from_title("some random title"), None);
    // No extension or slash → None
    assert_eq!(extract_path_from_title("Editing README"), None);
}

#[test]
fn title_fallback_in_extract_file_path() {
    let empty_input = serde_json::json!({});
    let no_locations: Vec<crate::ToolLocation> = vec![];
    // When input and locations are empty, falls back to title
    let result = extract_file_path(
        &empty_input,
        &no_locations,
        "Creating .magistr/current-plan.md",
    );
    assert_eq!(result, Some(".magistr/current-plan.md".to_string()));
}

#[test]
fn kind_other_with_title_falls_through_to_file_write() {
    // ACP may send kind=Other with raw_input=None for file creates.
    // The kind-based dispatch returns None for Other, and the name-based
    // dispatch won't match either. Phase 3 title-based fallback must catch it.
    use crate::ToolKind as ToolUseKind;
    let input = serde_json::Value::Null;
    let no_locs: Vec<crate::ToolLocation> = vec![];
    let result = tool_event_from(
        Some(ToolUseKind::Other),
        "Creating .magistr/current-plan.md",
        &input,
        &no_locs,
    );
    assert!(matches!(result, Some(ToolEvent::FileWrite { .. })));
    if let Some(ToolEvent::FileWrite { path }) = result {
        assert!(path.contains("current-plan.md"));
    }
}

#[test]
fn kind_edit_null_input_uses_title() {
    // ACP sends kind=Edit but raw_input=None on initial ToolCall.
    // extract_file_path should use title as fallback.
    use crate::ToolKind as ToolUseKind;
    let input = serde_json::Value::Null;
    let no_locs: Vec<crate::ToolLocation> = vec![];
    let result = tool_event_from(
        Some(ToolUseKind::Edit),
        "Editing src/main.rs",
        &input,
        &no_locs,
    );
    assert!(matches!(result, Some(ToolEvent::FileWrite { .. })));
    if let Some(ToolEvent::FileWrite { path }) = result {
        assert_eq!(path, "src/main.rs");
    }
}

#[test]
fn execute_empty_input_falls_through_to_title() {
    // Execute with empty input: kind dispatch returns None because cmd is empty,
    // but Phase 3 title-based fallback catches "Running cargo test".
    use crate::ToolKind as ToolUseKind;
    let input = serde_json::Value::Null;
    let no_locs: Vec<crate::ToolLocation> = vec![];
    let result = tool_event_from(
        Some(ToolUseKind::Execute),
        "Running cargo test",
        &input,
        &no_locs,
    );
    assert!(matches!(result, Some(ToolEvent::BashCommand { .. })));
}

#[test]
fn title_viewing_is_exploration() {
    use crate::ToolKind as ToolUseKind;
    let input = serde_json::Value::Null;
    let no_locs: Vec<crate::ToolLocation> = vec![];
    let result = tool_event_from(
        Some(ToolUseKind::Other),
        "Viewing /workspace/project/src/main.rs",
        &input,
        &no_locs,
    );
    assert!(matches!(result, Some(ToolEvent::Exploration { .. })));
}

#[test]
fn title_searching_is_research() {
    use crate::ToolKind as ToolUseKind;
    let input = serde_json::Value::Null;
    let no_locs: Vec<crate::ToolLocation> = vec![];
    let result = tool_event_from(
        Some(ToolUseKind::Other),
        "Searching for 'Rust best practices 2025'",
        &input,
        &no_locs,
    );
    assert!(matches!(result, Some(ToolEvent::McpToolCall { .. })));
}

#[test]
fn title_cargo_command_is_bash() {
    use crate::ToolKind as ToolUseKind;
    let input = serde_json::Value::Null;
    let no_locs: Vec<crate::ToolLocation> = vec![];
    let result = tool_event_from(
        Some(ToolUseKind::Other),
        "Check baseline build status cd /workspace && cargo clippy --workspace",
        &input,
        &no_locs,
    );
    assert!(matches!(result, Some(ToolEvent::BashCommand { .. })));
}

#[test]
fn from_name_parses_all_phases() {
    assert_eq!(
        WorkflowPhase::from_name("research"),
        Some(WorkflowPhase::Researching)
    );
    assert_eq!(
        WorkflowPhase::from_name("researching"),
        Some(WorkflowPhase::Researching)
    );
    assert_eq!(
        WorkflowPhase::from_name("plan"),
        Some(WorkflowPhase::Planning)
    );
    assert_eq!(
        WorkflowPhase::from_name("planning"),
        Some(WorkflowPhase::Planning)
    );
    assert_eq!(
        WorkflowPhase::from_name("execute"),
        Some(WorkflowPhase::Executing)
    );
    assert_eq!(
        WorkflowPhase::from_name("executing"),
        Some(WorkflowPhase::Executing)
    );
    assert_eq!(
        WorkflowPhase::from_name("verify"),
        Some(WorkflowPhase::Verifying)
    );
    assert_eq!(
        WorkflowPhase::from_name("verifying"),
        Some(WorkflowPhase::Verifying)
    );
    assert_eq!(
        WorkflowPhase::from_name("done"),
        Some(WorkflowPhase::Complete)
    );
    assert_eq!(
        WorkflowPhase::from_name("complete"),
        Some(WorkflowPhase::Complete)
    );
    assert_eq!(
        WorkflowPhase::from_name("RESEARCH"),
        Some(WorkflowPhase::Researching)
    );
    assert_eq!(WorkflowPhase::from_name("unknown"), None);
    assert_eq!(WorkflowPhase::from_name(""), None);
}

#[test]
fn force_phase_bypasses_research_guard() {
    let mut tracker = WorkflowTracker::new(None);
    assert_eq!(tracker.phase(), WorkflowPhase::Researching);

    // force_phase always succeeds — even without research evidence.
    // It's an explicit directive from the orchestration layer.
    let steer = tracker.force_phase(WorkflowPhase::Executing);
    assert!(steer.is_none(), "force_phase should always succeed");
    assert_eq!(tracker.phase(), WorkflowPhase::Executing);

    // Can go backwards too (explicit > evidence)
    let steer = tracker.force_phase(WorkflowPhase::Planning);
    assert!(steer.is_none());
    assert_eq!(tracker.phase(), WorkflowPhase::Planning);

    // Can jump to complete
    let steer = tracker.force_phase(WorkflowPhase::Complete);
    assert!(steer.is_none());
    assert_eq!(tracker.phase(), WorkflowPhase::Complete);
}

// ---- ship_plan as plan evidence (ts2c: the plan lives in the execution trace) ----

#[test]
fn ship_plan_call_after_research_transitions_to_planning() {
    let mut t = tracker();
    t.observe(&ToolEvent::McpToolCall {
        tool_name: "web_search".into(),
    });
    t.observe(&ToolEvent::ShipPlan {
        tool_name: "mcp__magistr__ship_plan".into(),
    });
    assert_eq!(t.phase(), WorkflowPhase::Planning);
    assert!(t.plan_evidence().plan_recorded);
    assert!(!t.plan_evidence().plan_file_written);
    assert!(t.plan_evidence().has_plan());
}

#[test]
fn ship_plan_then_source_edit_transitions_to_executing_without_plan_file() {
    let mut t = tracker();
    t.observe(&ToolEvent::McpToolCall {
        tool_name: "web_search".into(),
    });
    t.observe(&ToolEvent::ShipPlan {
        tool_name: "ship_set_objective".into(),
    });
    assert_eq!(t.phase(), WorkflowPhase::Planning);
    t.observe(&ToolEvent::FileWrite {
        path: "src/main.rs".into(),
    });
    assert_eq!(t.phase(), WorkflowPhase::Executing);
}

#[test]
fn ship_plan_observed_while_planning_counts() {
    let mut t = tracker();
    t.observe(&ToolEvent::McpToolCall {
        tool_name: "web_search".into(),
    });
    t.observe(&ToolEvent::PlanUpdate {
        classified_phase: Some(WorkflowPhase::Planning),
        all_complete: false,
    });
    assert_eq!(t.phase(), WorkflowPhase::Planning);
    t.observe(&ToolEvent::ShipPlan {
        tool_name: "magistr-ship_plan".into(),
    });
    assert!(t.plan_evidence().plan_recorded);
    t.observe(&ToolEvent::FileWrite {
        path: "src/lib.rs".into(),
    });
    assert_eq!(t.phase(), WorkflowPhase::Executing);
}

#[test]
fn tool_event_from_recognizes_ship_plan_verbs_under_any_prefix() {
    for name in [
        "ship_plan",
        "ship_set_objective",
        "mcp__magistr__ship_plan",
        "magistr-ship_set_objective",
    ] {
        let event = tool_event_from(
            Some(crate::ToolKind::Other),
            name,
            &serde_json::json!({"task_id": "t1", "title": "x"}),
            &[],
        );
        assert!(
            matches!(event, Some(ToolEvent::ShipPlan { .. })),
            "{name} → {event:?}"
        );
    }
    // Other ship verbs are not plan evidence.
    let event = tool_event_from(
        Some(crate::ToolKind::Other),
        "ship_start",
        &serde_json::json!({"task_id": "t1"}),
        &[],
    );
    assert!(
        !matches!(event, Some(ToolEvent::ShipPlan { .. })),
        "{event:?}"
    );
}
