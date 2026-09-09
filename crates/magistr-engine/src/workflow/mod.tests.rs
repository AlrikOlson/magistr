use super::*;

fn test_config() -> WorkflowConfig {
    WorkflowConfig {
        project_dir: "/tmp/test".into(),
        spec_path: "ROADMAP.md".into(),
        quality_gate: Some(QualityGateConfig {
            check: "cargo check".into(),
            test: "cargo test".into(),
            lint: "cargo clippy".into(),
        }),
        max_iterations: 10,
        lifecycle_hooks: vec![],
    }
}

// ── WorkflowEngine tests ────────────────────────────────────────────

#[test]
fn new_engine_starts_researching() {
    let engine = WorkflowEngine::new(test_config());
    assert_eq!(engine.phase(), WorkflowPhase::Researching);
    assert_eq!(engine.iteration().iteration, 0);
    assert_eq!(engine.consecutive_no_commits(), 0);
}

#[test]
fn advance_phase_forward() {
    let mut engine = WorkflowEngine::new(test_config());
    assert!(engine.advance_phase(WorkflowPhase::Planning).is_ok());
    assert_eq!(engine.phase(), WorkflowPhase::Planning);
    assert!(engine.advance_phase(WorkflowPhase::Executing).is_ok());
    assert_eq!(engine.phase(), WorkflowPhase::Executing);
}

#[test]
fn advance_phase_skip_allowed() {
    let mut engine = WorkflowEngine::new(test_config());
    assert!(engine.advance_phase(WorkflowPhase::Executing).is_ok());
    assert_eq!(engine.phase(), WorkflowPhase::Executing);
}

#[test]
fn advance_phase_idempotent() {
    let mut engine = WorkflowEngine::new(test_config());
    engine.advance_phase(WorkflowPhase::Planning).unwrap();
    assert!(engine.advance_phase(WorkflowPhase::Planning).is_ok());
    assert_eq!(engine.phase(), WorkflowPhase::Planning);
}

#[test]
fn advance_phase_regression_rejected() {
    let mut engine = WorkflowEngine::new(test_config());
    engine.advance_phase(WorkflowPhase::Executing).unwrap();
    let result = engine.advance_phase(WorkflowPhase::Planning);
    assert!(result.is_err());
    assert_eq!(engine.phase(), WorkflowPhase::Executing);
}

#[test]
fn force_phase_bypasses_validation() {
    let mut engine = WorkflowEngine::new(test_config());
    engine.advance_phase(WorkflowPhase::Verifying).unwrap();
    engine.force_phase(WorkflowPhase::Researching);
    assert_eq!(engine.phase(), WorkflowPhase::Researching);
}

#[test]
fn reset_for_next_iteration() {
    let mut engine = WorkflowEngine::new(test_config());
    engine.advance_phase(WorkflowPhase::Complete).unwrap();
    engine.reset_for_next_iteration();
    assert_eq!(engine.phase(), WorkflowPhase::Researching);
}

#[test]
fn spec_complete_detection() {
    let mut engine = WorkflowEngine::new(test_config());
    assert!(!engine.is_spec_complete());
    engine.update_spec_progress(10, 10);
    assert!(engine.is_spec_complete());
}

#[test]
fn spec_not_complete_when_zero_total() {
    let mut engine = WorkflowEngine::new(test_config());
    engine.update_spec_progress(0, 0);
    assert!(!engine.is_spec_complete());
}

#[test]
fn iteration_limit_detection() {
    let config = WorkflowConfig {
        max_iterations: 2,
        ..test_config()
    };
    let mut engine = WorkflowEngine::new(config);
    assert!(!engine.is_iteration_limit_reached());
    engine.advance_iteration();
    assert!(!engine.is_iteration_limit_reached());
    engine.advance_iteration();
    assert!(engine.is_iteration_limit_reached());
}

#[test]
fn no_commit_tracking() {
    let mut engine = WorkflowEngine::new(test_config());
    engine.record_no_commit();
    assert_eq!(engine.consecutive_no_commits(), 1);
    engine.record_no_commit();
    assert_eq!(engine.consecutive_no_commits(), 2);
    engine.record_committed();
    assert_eq!(engine.consecutive_no_commits(), 0);
}

#[test]
fn should_stop_after_max_no_commits() {
    let mut engine = WorkflowEngine::new(test_config());
    assert!(!engine.should_stop_no_commits());
    engine.record_no_commit();
    engine.record_no_commit();
    engine.record_no_commit();
    assert!(engine.should_stop_no_commits());
}

#[test]
fn should_stop_combines_conditions() {
    let mut engine = WorkflowEngine::new(test_config());
    assert!(!engine.should_stop());

    // Spec complete triggers stop
    engine.update_spec_progress(5, 5);
    assert!(engine.should_stop());
}

#[test]
fn advance_iteration_resets_phase() {
    let mut engine = WorkflowEngine::new(test_config());
    engine.advance_phase(WorkflowPhase::Complete).unwrap();
    engine.advance_iteration();
    assert_eq!(engine.iteration().iteration, 1);
    assert_eq!(engine.phase(), WorkflowPhase::Researching);
}

#[test]
fn display_iteration_is_one_based() {
    let mut engine = WorkflowEngine::new(test_config());
    assert_eq!(engine.display_iteration(), 1);
    engine.advance_iteration();
    assert_eq!(engine.display_iteration(), 2);
}

#[test]
fn record_failed_sets_status() {
    let mut engine = WorkflowEngine::new(test_config());
    engine.record_failed();
    assert_eq!(engine.iteration().last_status, IterationStatus::Failed);
}

#[test]
fn start_iteration_valid_from_researching() {
    let engine = WorkflowEngine::new(test_config());
    assert!(
        engine
            .validate_command(&WorkflowCommand::StartIteration)
            .is_ok()
    );
}

#[test]
fn start_iteration_valid_from_complete() {
    let mut engine = WorkflowEngine::new(test_config());
    engine.advance_phase(WorkflowPhase::Complete).unwrap();
    assert!(
        engine
            .validate_command(&WorkflowCommand::StartIteration)
            .is_ok()
    );
}

#[test]
fn start_iteration_rejected_mid_workflow() {
    let mut engine = WorkflowEngine::new(test_config());
    engine.advance_phase(WorkflowPhase::Executing).unwrap();
    assert!(
        engine
            .validate_command(&WorkflowCommand::StartIteration)
            .is_err()
    );
}

#[test]
fn pause_valid_when_running() {
    let engine = WorkflowEngine::new(test_config());
    assert!(engine.validate_command(&WorkflowCommand::Pause).is_ok());
}

#[test]
fn pause_rejected_when_already_paused() {
    let mut engine = WorkflowEngine::new(test_config());
    engine.execute_command(&WorkflowCommand::Pause).unwrap();
    assert!(engine.validate_command(&WorkflowCommand::Pause).is_err());
}

#[test]
fn resume_valid_when_paused() {
    let mut engine = WorkflowEngine::new(test_config());
    engine.execute_command(&WorkflowCommand::Pause).unwrap();
    assert!(engine.validate_command(&WorkflowCommand::Resume).is_ok());
}

#[test]
fn resume_rejected_when_not_paused() {
    let engine = WorkflowEngine::new(test_config());
    assert!(engine.validate_command(&WorkflowCommand::Resume).is_err());
}

#[test]
fn abort_sets_aborted_flag() {
    let mut engine = WorkflowEngine::new(test_config());
    engine.execute_command(&WorkflowCommand::AbortLoop).unwrap();
    assert!(engine.is_aborted());
    assert!(engine.should_stop());
}

#[test]
fn abort_rejects_all_commands_except_resume() {
    let mut engine = WorkflowEngine::new(test_config());
    engine.execute_command(&WorkflowCommand::AbortLoop).unwrap();
    assert!(
        engine
            .validate_command(&WorkflowCommand::StartIteration)
            .is_err()
    );
    assert!(engine.validate_command(&WorkflowCommand::Pause).is_err());
    assert!(engine.validate_command(&WorkflowCommand::Skip).is_err());
    assert!(
        engine
            .validate_command(&WorkflowCommand::AbortLoop)
            .is_err()
    );
}

#[test]
fn skip_advances_iteration() {
    let mut engine = WorkflowEngine::new(test_config());
    let iter_before = engine.iteration().iteration;
    engine.execute_command(&WorkflowCommand::Skip).unwrap();
    assert_eq!(engine.iteration().iteration, iter_before + 1);
    assert_eq!(engine.consecutive_no_commits(), 1);
}

#[test]
fn pause_then_resume_clears_paused() {
    let mut engine = WorkflowEngine::new(test_config());
    engine.execute_command(&WorkflowCommand::Pause).unwrap();
    assert!(engine.is_paused());
    assert!(engine.should_stop());
    engine.execute_command(&WorkflowCommand::Resume).unwrap();
    assert!(!engine.is_paused());
    assert!(!engine.should_stop());
}

#[test]
fn start_iteration_from_complete_advances() {
    let mut engine = WorkflowEngine::new(test_config());
    engine.advance_phase(WorkflowPhase::Complete).unwrap();
    engine
        .execute_command(&WorkflowCommand::StartIteration)
        .unwrap();
    assert_eq!(engine.phase(), WorkflowPhase::Researching);
    assert_eq!(engine.iteration().iteration, 1);
}

#[test]
fn engine_serialization_roundtrip() {
    let mut engine = WorkflowEngine::new(test_config());
    engine.advance_phase(WorkflowPhase::Executing).unwrap();
    engine.update_spec_progress(3, 10);
    engine.record_committed();

    let json = serde_json::to_string(&engine).unwrap();
    let deserialized: WorkflowEngine = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.phase(), WorkflowPhase::Executing);
    assert_eq!(deserialized.iteration().spec_checked, 3);
    assert_eq!(
        deserialized.iteration().last_status,
        IterationStatus::Committed
    );
}

// ── Lifecycle hook serde tests ───────────────────────────────────────

#[test]
fn lifecycle_event_serde_roundtrip() {
    let events = vec![
        LifecycleEvent::OnEnter(WorkflowPhase::Researching),
        LifecycleEvent::OnExit(WorkflowPhase::Verifying),
        LifecycleEvent::OnIterationStart,
        LifecycleEvent::OnIterationEnd,
        LifecycleEvent::OnGatePass,
        LifecycleEvent::OnGateFail,
    ];
    for event in &events {
        let json = serde_json::to_string(event).unwrap();
        let parsed: LifecycleEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(&parsed, event);
    }
}

#[test]
fn lifecycle_hook_serde_roundtrip_minimal() {
    let hook = LifecycleHook {
        event: LifecycleEvent::OnGatePass,
        action: HookAction::McpTool {
            tool: "brave_web_search".to_owned(),
            arguments: serde_json::Value::Null,
        },
        description: None,
        on_failure: OnFailure::default(),
    };
    let json = serde_json::to_string(&hook).unwrap();
    let parsed: LifecycleHook = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, hook);
    // Null arguments, None description, and default on_failure should be omitted
    assert!(!json.contains("arguments"));
    assert!(!json.contains("description"));
    assert!(!json.contains("on_failure"));
}

#[test]
fn lifecycle_hook_serde_roundtrip_full() {
    let hook = LifecycleHook {
        event: LifecycleEvent::OnEnter(WorkflowPhase::Executing),
        action: HookAction::McpTool {
            tool: "scope_search".to_owned(),
            arguments: serde_json::json!({"query": "test helpers"}),
        },
        description: Some("search for test helpers on phase entry".to_owned()),
        on_failure: OnFailure::default(),
    };
    let json = serde_json::to_string(&hook).unwrap();
    let parsed: LifecycleHook = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, hook);
}

#[test]
fn lifecycle_hook_shell_command_serde_roundtrip() {
    let hook = LifecycleHook {
        event: LifecycleEvent::OnEnter(WorkflowPhase::Verifying),
        action: HookAction::ShellCommand {
            command: "cargo fmt".to_owned(),
            timeout_secs: Some(60),
        },
        description: Some("format code before verify".to_owned()),
        on_failure: OnFailure::FailFast,
    };
    let json = serde_json::to_string(&hook).unwrap();
    let parsed: LifecycleHook = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, hook);
    assert!(json.contains("command"));
    assert!(json.contains("cargo fmt"));
    assert!(json.contains("timeout_secs"));
}

#[test]
fn lifecycle_hooks_config_empty() {
    let config = LifecycleHooksConfig::empty();
    assert!(config.is_empty());
    assert_eq!(config.len(), 0);
    let json = serde_json::to_string(&config).unwrap();
    assert_eq!(json, "[]");
    let parsed: LifecycleHooksConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, config);
}

#[test]
fn lifecycle_hooks_config_multiple_hooks() {
    let hooks = vec![
        LifecycleHook {
            event: LifecycleEvent::OnEnter(WorkflowPhase::Researching),
            action: HookAction::McpTool {
                tool: "brave_web_search".to_owned(),
                arguments: serde_json::json!({"query": "best practices"}),
            },
            description: None,
            on_failure: OnFailure::default(),
        },
        LifecycleHook {
            event: LifecycleEvent::OnGatePass,
            action: HookAction::McpTool {
                tool: "scope_search".to_owned(),
                arguments: serde_json::Value::Null,
            },
            description: Some("celebrate".to_owned()),
            on_failure: OnFailure::default(),
        },
    ];
    let config = LifecycleHooksConfig::new(hooks).unwrap();
    assert_eq!(config.len(), 2);
    let json = serde_json::to_string(&config).unwrap();
    let parsed: LifecycleHooksConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.len(), 2);
}

#[test]
fn lifecycle_hooks_config_rejects_duplicates() {
    let hooks = vec![
        LifecycleHook {
            event: LifecycleEvent::OnGatePass,
            action: HookAction::McpTool {
                tool: "my_tool".to_owned(),
                arguments: serde_json::Value::Null,
            },
            description: None,
            on_failure: OnFailure::default(),
        },
        LifecycleHook {
            event: LifecycleEvent::OnGatePass,
            action: HookAction::McpTool {
                tool: "my_tool".to_owned(),
                arguments: serde_json::json!({"different": true}),
            },
            description: None,
            on_failure: OnFailure::default(),
        },
    ];
    let result = LifecycleHooksConfig::new(hooks);
    assert!(result.is_err());
    assert!(result.unwrap_err().message.contains("duplicate hook"));
}

#[test]
fn lifecycle_hooks_config_allows_same_event_different_tools() {
    let hooks = vec![
        LifecycleHook {
            event: LifecycleEvent::OnGatePass,
            action: HookAction::McpTool {
                tool: "tool_a".to_owned(),
                arguments: serde_json::Value::Null,
            },
            description: None,
            on_failure: OnFailure::default(),
        },
        LifecycleHook {
            event: LifecycleEvent::OnGatePass,
            action: HookAction::McpTool {
                tool: "tool_b".to_owned(),
                arguments: serde_json::Value::Null,
            },
            description: None,
            on_failure: OnFailure::default(),
        },
    ];
    assert!(LifecycleHooksConfig::new(hooks).is_ok());
}

#[test]
fn lifecycle_hooks_config_allows_same_tool_different_events() {
    let hooks = vec![
        LifecycleHook {
            event: LifecycleEvent::OnGatePass,
            action: HookAction::McpTool {
                tool: "my_tool".to_owned(),
                arguments: serde_json::Value::Null,
            },
            description: None,
            on_failure: OnFailure::default(),
        },
        LifecycleHook {
            event: LifecycleEvent::OnGateFail,
            action: HookAction::McpTool {
                tool: "my_tool".to_owned(),
                arguments: serde_json::Value::Null,
            },
            description: None,
            on_failure: OnFailure::default(),
        },
    ];
    assert!(LifecycleHooksConfig::new(hooks).is_ok());
}

#[test]
fn workflow_config_lifecycle_hooks_default_empty() {
    let json = r#"{
        "project_dir": "/tmp",
        "spec_path": "SPEC.md",
        "quality_gate": null,
        "max_iterations": 5
    }"#;
    let config: WorkflowConfig = serde_json::from_str(json).unwrap();
    assert!(config.lifecycle_hooks.is_empty());
}

#[test]
fn workflow_config_lifecycle_hooks_roundtrip() {
    let config = WorkflowConfig {
        project_dir: "/tmp".into(),
        spec_path: "SPEC.md".into(),
        quality_gate: None,
        max_iterations: 5,
        lifecycle_hooks: vec![LifecycleHook {
            event: LifecycleEvent::OnIterationStart,
            action: HookAction::McpTool {
                tool: "my_tool".to_owned(),
                arguments: serde_json::json!({"key": "val"}),
            },
            description: None,
            on_failure: OnFailure::default(),
        }],
    };
    let json = serde_json::to_string(&config).unwrap();
    let parsed: WorkflowConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.lifecycle_hooks.len(), 1);
    assert_eq!(parsed.lifecycle_hooks[0].display_name(), "my_tool");
}

#[test]
fn lifecycle_event_display() {
    assert_eq!(
        LifecycleEvent::OnEnter(WorkflowPhase::Planning).to_string(),
        "on_enter(Planning)"
    );
    assert_eq!(
        LifecycleEvent::OnIterationStart.to_string(),
        "on_iteration_start"
    );
    assert_eq!(LifecycleEvent::OnGateFail.to_string(), "on_gate_fail");
}

#[test]
fn lifecycle_event_unknown_json_field_graceful() {
    // Unknown fields in the event should be handled gracefully.
    // Since we use tagged enum, unknown variants will error — but unknown
    // fields within a known variant should be fine with default serde behavior.
    let json = r#"{"type":"on_gate_pass"}"#;
    let event: LifecycleEvent = serde_json::from_str(json).unwrap();
    assert_eq!(event, LifecycleEvent::OnGatePass);
}

// ── LifecycleHooksConfig add/remove tests ────────────────────────────

fn make_hook(event: LifecycleEvent, tool: &str) -> LifecycleHook {
    LifecycleHook {
        event,
        action: HookAction::McpTool {
            tool: tool.to_owned(),
            arguments: serde_json::Value::Null,
        },
        description: None,
        on_failure: OnFailure::default(),
    }
}

#[test]
fn hooks_config_add_succeeds() {
    let mut config = LifecycleHooksConfig::empty();
    let hook = make_hook(LifecycleEvent::OnGatePass, "my_tool");
    assert!(config.add(hook).is_ok());
    assert_eq!(config.len(), 1);
}

#[test]
fn hooks_config_add_rejects_duplicate() {
    let mut config = LifecycleHooksConfig::empty();
    let hook1 = make_hook(LifecycleEvent::OnGatePass, "my_tool");
    let hook2 = make_hook(LifecycleEvent::OnGatePass, "my_tool");
    assert!(config.add(hook1).is_ok());
    let err = config.add(hook2).unwrap_err();
    assert!(err.message.contains("duplicate"));
    assert_eq!(config.len(), 1);
}

#[test]
fn hooks_config_add_allows_different_event_same_tool() {
    let mut config = LifecycleHooksConfig::empty();
    assert!(
        config
            .add(make_hook(LifecycleEvent::OnGatePass, "my_tool"))
            .is_ok()
    );
    assert!(
        config
            .add(make_hook(LifecycleEvent::OnGateFail, "my_tool"))
            .is_ok()
    );
    assert_eq!(config.len(), 2);
}

#[test]
fn hooks_config_remove_by_index() {
    let mut config = LifecycleHooksConfig::new(vec![
        make_hook(LifecycleEvent::OnGatePass, "tool_a"),
        make_hook(LifecycleEvent::OnGateFail, "tool_b"),
    ])
    .unwrap();
    let removed = config.remove_by_index(0).unwrap();
    assert_eq!(removed.display_name(), "tool_a");
    assert_eq!(config.len(), 1);
    assert_eq!(config.hooks()[0].display_name(), "tool_b");
}

#[test]
fn hooks_config_remove_by_index_out_of_bounds() {
    let mut config = LifecycleHooksConfig::empty();
    let err = config.remove_by_index(0).unwrap_err();
    assert!(err.message.contains("out of bounds"));
}

#[test]
fn hooks_config_remove_by_event_and_name() {
    let mut config = LifecycleHooksConfig::new(vec![
        make_hook(LifecycleEvent::OnGatePass, "tool_a"),
        make_hook(LifecycleEvent::OnGateFail, "tool_b"),
    ])
    .unwrap();
    let removed = config
        .remove_by_event_and_name(&LifecycleEvent::OnGateFail, "tool_b")
        .unwrap();
    assert_eq!(removed.display_name(), "tool_b");
    assert_eq!(config.len(), 1);
}

#[test]
fn hooks_config_remove_by_event_and_name_not_found() {
    let mut config = LifecycleHooksConfig::empty();
    let err = config
        .remove_by_event_and_name(&LifecycleEvent::OnGatePass, "nonexistent")
        .unwrap_err();
    assert!(err.message.contains("no hook found"));
}

// ── Security validation tests ────────────────────────────────────────

#[test]
fn shell_command_rejects_path_traversal() {
    use crate::workflow::lifecycle_hooks_config::validate_shell_command;
    let err = validate_shell_command("cat ../../../etc/passwd").unwrap_err();
    assert!(err.message.contains("path traversal"));
}

#[test]
fn shell_command_rejects_protected_dirs() {
    use crate::workflow::lifecycle_hooks_config::validate_shell_command;
    let err = validate_shell_command("ls /etc/shadow").unwrap_err();
    assert!(err.message.contains("protected directory"));
}

#[test]
fn shell_command_rejects_dangerous_patterns() {
    use crate::workflow::lifecycle_hooks_config::validate_shell_command;
    let err = validate_shell_command("rm -rf /").unwrap_err();
    assert!(err.message.contains("dangerous pattern"));
}

#[test]
fn shell_command_allows_safe_commands() {
    use crate::workflow::lifecycle_hooks_config::validate_shell_command;
    assert!(validate_shell_command("cargo fmt").is_ok());
    assert!(validate_shell_command("npm run lint").is_ok());
    assert!(validate_shell_command("python -m black .").is_ok());
}

#[test]
fn hooks_config_rejects_dangerous_shell_hook() {
    let hook = LifecycleHook {
        event: LifecycleEvent::OnGatePass,
        action: HookAction::ShellCommand {
            command: "rm -rf /".to_owned(),
            timeout_secs: None,
        },
        description: None,
        on_failure: OnFailure::default(),
    };
    let result = LifecycleHooksConfig::new(vec![hook]);
    assert!(result.is_err());
    assert!(result.unwrap_err().message.contains("dangerous"));
}

#[test]
fn hooks_config_add_rejects_dangerous_shell_hook() {
    let mut config = LifecycleHooksConfig::empty();
    let hook = LifecycleHook {
        event: LifecycleEvent::OnGatePass,
        action: HookAction::ShellCommand {
            command: "cat ../../../etc/passwd".to_owned(),
            timeout_secs: None,
        },
        description: None,
        on_failure: OnFailure::default(),
    };
    let err = config.add(hook).unwrap_err();
    assert!(err.message.contains("path traversal"));
}
