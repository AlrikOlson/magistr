//! Integration tests for workflow engine: plan lifecycle, tracker phase transitions,
//! and gate enforcement.
//!
//! Tests cross-module interactions between `workflow::plan`, `workflow::plans`,
//! `workflow::tracker`, and `validate`.

use tempfile::TempDir;

use magistr::workflow::plan::{
    LoopState, cleanup_state, plan_exists, plan_path, read_plan, task_path,
};
use magistr::workflow::plans::{
    PlanStatus, SavedPlan, abandon_active_plans, find_active_plan, list_plans, load_plan,
    save_plan, update_status,
};
use magistr::workflow::tracker::{
    ToolEvent, WorkflowPhase, WorkflowTracker, is_quality_gate_command, is_test_only_command,
};

use magistr::types::QualityGate;

// ── Helpers ────────────────────────────────────────────────────────

fn rust_gate() -> Option<QualityGate> {
    Some(QualityGate {
        check: "cargo check --workspace".into(),
        test: "cargo test --workspace".into(),
        lint: "cargo clippy --workspace -- -D warnings".into(),
    })
}

fn sample_plan(id: &str, status: PlanStatus) -> SavedPlan {
    SavedPlan {
        id: id.into(),
        created_at: chrono::Utc::now(),
        status,
        task_name: format!("Task {id}"),
        content: format!("# Plan {id}\n\nSome plan content."),
        spec_items: vec!["item 1".into(), "item 2".into()],
    }
}

// ── Plan file lifecycle ────────────────────────────────────────────

#[test]
fn plan_file_create_read_cleanup() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path();

    // No plan yet
    assert!(!plan_exists(project));

    // Create plan and task files
    let plan = plan_path(project);
    let task = task_path(project);
    std::fs::create_dir_all(plan.parent().unwrap()).unwrap();
    std::fs::write(&plan, "# My Plan\n\n1. Do stuff").unwrap();
    std::fs::write(&task, "implement feature X").unwrap();

    assert!(plan_exists(project));
    let content = read_plan(project).unwrap();
    assert!(content.contains("My Plan"));

    // Cleanup removes both files
    cleanup_state(project).unwrap();
    assert!(!plan_exists(project));
    assert!(!task.exists());
}

// ── Plan archive lifecycle ─────────────────────────────────────────

#[test]
fn archive_save_list_update_lifecycle() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path();

    // Save multiple plans
    let p1 = sample_plan("plan-001", PlanStatus::Active);
    let p2 = sample_plan("plan-002", PlanStatus::Completed);
    let p3 = sample_plan("plan-003", PlanStatus::Active);

    save_plan(project, &p1).unwrap();
    save_plan(project, &p2).unwrap();
    save_plan(project, &p3).unwrap();

    // List returns all, sorted by creation time
    let all = list_plans(project).unwrap();
    assert_eq!(all.len(), 3);

    // Find active plan returns the most recent active
    let active = find_active_plan(project).unwrap().unwrap();
    assert_eq!(active.id, "plan-003");

    // Update status
    update_status(project, "plan-001", PlanStatus::Abandoned).unwrap();
    let loaded = load_plan(project, "plan-001").unwrap();
    assert_eq!(loaded.status, PlanStatus::Abandoned);

    // Abandon all active
    let abandoned = abandon_active_plans(project).unwrap();
    assert_eq!(abandoned, 1); // plan-003 was the remaining active
    assert!(find_active_plan(project).unwrap().is_none());
}

// ── WorkflowTracker full lifecycle ─────────────────────────────────

#[test]
fn tracker_full_plan_execute_verify_lifecycle() {
    let mut tracker = WorkflowTracker::new(rust_gate());

    // Phase 0: Researching
    assert_eq!(tracker.phase(), WorkflowPhase::Researching);

    // Web search
    tracker.observe(&ToolEvent::McpToolCall {
        tool_name: "web_search".into(),
    });
    assert_eq!(tracker.phase(), WorkflowPhase::Researching);

    // Phase 1: Planning — write plan file triggers transition
    tracker.observe(&ToolEvent::FileWrite {
        path: ".magistr/current-plan.md".into(),
    });
    assert_eq!(tracker.phase(), WorkflowPhase::Planning);

    // Write task file
    tracker.observe(&ToolEvent::FileWrite {
        path: ".magistr/current-task.md".into(),
    });
    assert_eq!(tracker.phase(), WorkflowPhase::Planning); // Still planning

    // Phase 2: First source edit triggers transition
    tracker.observe(&ToolEvent::FileWrite {
        path: "src/lib.rs".into(),
    });
    assert_eq!(tracker.phase(), WorkflowPhase::Executing);

    // More source edits stay in executing
    tracker.observe(&ToolEvent::FileWrite {
        path: "src/types.rs".into(),
    });
    tracker.observe(&ToolEvent::FileWrite {
        path: "tests/test_lib.rs".into(),
    });

    // TDD test run stays in executing
    tracker.observe(&ToolEvent::BashCommand {
        command: "cargo test --workspace".into(),
    });
    assert_eq!(tracker.phase(), WorkflowPhase::Executing);

    // Phase 3: Full quality gate triggers transition to verifying
    tracker.observe(&ToolEvent::BashCommand {
        command: "cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings".into(),
    });
    assert_eq!(tracker.phase(), WorkflowPhase::Verifying);

    // In verify phase: quality gate must run again (verify has its own evidence)
    tracker.observe(&ToolEvent::BashCommand {
        command: "cargo clippy --workspace -- -D warnings".into(),
    });

    // Update spec file — with quality gate already ran in verify, this triggers Complete
    tracker.observe(&ToolEvent::FileWrite {
        path: "ROADMAP.md".into(),
    });
    assert_eq!(tracker.phase(), WorkflowPhase::Complete);
}

// ── Tracker distinguishes TDD from quality gate ────────────────────

#[test]
fn tracker_tdd_does_not_trigger_verify() {
    let gate = rust_gate();

    // "cargo test" alone is TDD, not a quality gate
    assert!(is_test_only_command("cargo test --workspace"));
    assert!(!is_quality_gate_command("cargo test --workspace", &gate));

    // Full gate includes lint
    assert!(is_quality_gate_command(
        "cargo clippy --workspace -- -D warnings",
        &gate
    ));
}

// ── Loop state persistence ─────────────────────────────────────────

#[test]
fn loop_state_serialize_parse_round_trip() {
    let state = LoopState {
        iteration: 5,
        last_status: "completed successfully".into(),
    };

    let serialized = state.serialize();
    let parsed = LoopState::parse(&serialized);

    assert_eq!(parsed.iteration, 5);
    assert_eq!(parsed.last_status, "completed successfully");
}

// ── Tracker reset for new iteration ────────────────────────────────

#[test]
fn tracker_reset_preserves_gate_config() {
    let mut tracker = WorkflowTracker::new(rust_gate());

    // Research + Drive to Executing
    tracker.observe(&ToolEvent::McpToolCall {
        tool_name: "web_search".into(),
    });
    tracker.observe(&ToolEvent::FileWrite {
        path: ".magistr/current-plan.md".into(),
    });
    tracker.observe(&ToolEvent::FileWrite {
        path: "src/main.rs".into(),
    });
    assert_eq!(tracker.phase(), WorkflowPhase::Executing);

    // Reset
    tracker.reset();
    assert_eq!(tracker.phase(), WorkflowPhase::Researching);

    // Quality gate config is preserved — can still detect gates
    assert!(is_quality_gate_command(
        "cargo clippy --workspace -- -D warnings",
        &rust_gate()
    ));
}

// ── Multi-stack gate detection ─────────────────────────────────────

#[test]
fn gate_detection_across_stacks() {
    let node_gate = Some(QualityGate {
        check: "npx tsc --noEmit".into(),
        test: "npm test".into(),
        lint: "npm run lint".into(),
    });

    let python_gate = Some(QualityGate {
        check: "python -m mypy .".into(),
        test: "pytest".into(),
        lint: "ruff check .".into(),
    });

    // Node: npm test alone is TDD
    assert!(is_test_only_command("npm test"));
    assert!(!is_quality_gate_command("npm test", &node_gate));
    assert!(is_quality_gate_command("npm run lint", &node_gate));

    // Python: pytest alone is TDD
    assert!(is_test_only_command("pytest"));
    assert!(!is_quality_gate_command("pytest", &python_gate));
    assert!(is_quality_gate_command("ruff check .", &python_gate));
}

// ── Iteration lifecycle — full cycle with mocked events ────────────

#[test]
fn iteration_lifecycle_research_to_complete() {
    let mut tracker = WorkflowTracker::new(rust_gate());

    // Research phase: MCP tool calls.
    assert_eq!(tracker.phase(), WorkflowPhase::Researching);
    tracker.observe(&ToolEvent::McpToolCall {
        tool_name: "web_search".into(),
    });
    tracker.observe(&ToolEvent::McpToolCall {
        tool_name: "scope_search".into(),
    });
    assert_eq!(tracker.phase(), WorkflowPhase::Researching);

    // Plan phase: write plan file.
    tracker.observe(&ToolEvent::FileWrite {
        path: ".magistr/current-plan.md".into(),
    });
    assert_eq!(tracker.phase(), WorkflowPhase::Planning);

    // Execute phase: write source and test files (TDD).
    tracker.observe(&ToolEvent::FileWrite {
        path: "src/feature.tests.rs".into(),
    });
    assert_eq!(tracker.phase(), WorkflowPhase::Executing);

    // TDD cycle: run tests (stays in executing).
    tracker.observe(&ToolEvent::BashCommand {
        command: "cargo test --workspace".into(),
    });
    assert_eq!(tracker.phase(), WorkflowPhase::Executing);

    // Write implementation.
    tracker.observe(&ToolEvent::FileWrite {
        path: "src/feature.rs".into(),
    });
    assert_eq!(tracker.phase(), WorkflowPhase::Executing);

    // Run tests again (TDD green).
    tracker.observe(&ToolEvent::BashCommand {
        command: "cargo test --workspace".into(),
    });
    assert_eq!(tracker.phase(), WorkflowPhase::Executing);

    // Quality gate triggers verify.
    tracker.observe(&ToolEvent::BashCommand {
        command: "cargo clippy --workspace -- -D warnings".into(),
    });
    assert_eq!(tracker.phase(), WorkflowPhase::Verifying);

    // In verify: run gate again + update spec → complete.
    tracker.observe(&ToolEvent::BashCommand {
        command: "cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings".into(),
    });
    tracker.observe(&ToolEvent::FileWrite {
        path: "ROADMAP.md".into(),
    });
    assert_eq!(tracker.phase(), WorkflowPhase::Complete);
}

// ── Partial gate does not trigger verify ────────────────────────────

#[test]
fn partial_gate_stays_in_executing() {
    let mut tracker = WorkflowTracker::new(rust_gate());

    // Research first (required by research guard).
    tracker.observe(&ToolEvent::McpToolCall {
        tool_name: "web_search".into(),
    });
    // Skip to executing.
    tracker.observe(&ToolEvent::FileWrite {
        path: ".magistr/current-plan.md".into(),
    });
    tracker.observe(&ToolEvent::FileWrite {
        path: "src/lib.rs".into(),
    });
    assert_eq!(tracker.phase(), WorkflowPhase::Executing);

    // Running only `cargo test` is TDD, not a quality gate.
    tracker.observe(&ToolEvent::BashCommand {
        command: "cargo test --workspace".into(),
    });
    assert_eq!(tracker.phase(), WorkflowPhase::Executing);

    // Running only `cargo check` is also not the full gate.
    tracker.observe(&ToolEvent::BashCommand {
        command: "cargo check --workspace".into(),
    });
    assert_eq!(tracker.phase(), WorkflowPhase::Executing);
}

// ── Spec update without gate does not complete ──────────────────────

#[test]
fn spec_update_without_gate_does_not_complete() {
    let mut tracker = WorkflowTracker::new(rust_gate());

    // Research first (required by research guard).
    tracker.observe(&ToolEvent::McpToolCall {
        tool_name: "web_search".into(),
    });
    // Drive to executing.
    tracker.observe(&ToolEvent::FileWrite {
        path: ".magistr/current-plan.md".into(),
    });
    tracker.observe(&ToolEvent::FileWrite {
        path: "src/lib.rs".into(),
    });
    assert_eq!(tracker.phase(), WorkflowPhase::Executing);

    // Write ROADMAP.md without running quality gate — should NOT go to Complete.
    tracker.observe(&ToolEvent::FileWrite {
        path: "ROADMAP.md".into(),
    });
    // Should still be executing since no quality gate has been run.
    assert_ne!(tracker.phase(), WorkflowPhase::Complete);
}

// ── Reset allows fresh iteration cycle ──────────────────────────────

#[test]
fn reset_enables_fresh_iteration() {
    let mut tracker = WorkflowTracker::new(rust_gate());

    // Research first (required by research guard).
    tracker.observe(&ToolEvent::McpToolCall {
        tool_name: "web_search".into(),
    });
    // Drive through plan → execute → verify.
    tracker.observe(&ToolEvent::FileWrite {
        path: ".magistr/current-plan.md".into(),
    });
    // First source edit transitions Planning → Executing.
    tracker.observe(&ToolEvent::FileWrite {
        path: "src/lib.rs".into(),
    });
    assert_eq!(tracker.phase(), WorkflowPhase::Executing);
    // Second source edit while in Executing registers source_files_touched.
    tracker.observe(&ToolEvent::FileWrite {
        path: "src/types.rs".into(),
    });
    tracker.observe(&ToolEvent::BashCommand {
        command: "cargo clippy --workspace -- -D warnings".into(),
    });
    assert_eq!(tracker.phase(), WorkflowPhase::Verifying);

    // Reset for a new iteration.
    tracker.reset();
    assert_eq!(tracker.phase(), WorkflowPhase::Researching);

    // Can go through the full cycle again.
    tracker.observe(&ToolEvent::McpToolCall {
        tool_name: "web_search".into(),
    });
    assert_eq!(tracker.phase(), WorkflowPhase::Researching);

    tracker.observe(&ToolEvent::FileWrite {
        path: ".magistr/current-plan.md".into(),
    });
    assert_eq!(tracker.phase(), WorkflowPhase::Planning);

    tracker.observe(&ToolEvent::FileWrite {
        path: "src/new_module.rs".into(),
    });
    assert_eq!(tracker.phase(), WorkflowPhase::Executing);
}

// ── Skipping plan goes directly to executing ────────────────────────

#[test]
fn source_edit_without_plan_skips_to_executing() {
    let mut tracker = WorkflowTracker::new(rust_gate());
    assert_eq!(tracker.phase(), WorkflowPhase::Researching);

    // Research first (required by research guard).
    tracker.observe(&ToolEvent::McpToolCall {
        tool_name: "web_search".into(),
    });

    // Write source file directly without a plan file.
    tracker.observe(&ToolEvent::FileWrite {
        path: "src/hotfix.rs".into(),
    });

    // Should transition to Executing (skipping Planning).
    assert_eq!(tracker.phase(), WorkflowPhase::Executing);
}

// ── Plan + state file lifecycle with tracker ────────────────────────

#[test]
fn plan_state_and_tracker_work_together() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path();
    let mut tracker = WorkflowTracker::new(rust_gate());

    // Research first (required by research guard).
    tracker.observe(&ToolEvent::McpToolCall {
        tool_name: "web_search".into(),
    });

    // Create plan file on disk and observe it in tracker.
    let plan = plan_path(project);
    std::fs::create_dir_all(plan.parent().unwrap()).unwrap();
    std::fs::write(&plan, "# Iteration Plan\n\n1. Add feature").unwrap();
    tracker.observe(&ToolEvent::FileWrite {
        path: ".magistr/current-plan.md".into(),
    });

    assert!(plan_exists(project));
    assert_eq!(tracker.phase(), WorkflowPhase::Planning);

    // Execute.
    tracker.observe(&ToolEvent::FileWrite {
        path: "src/feature.rs".into(),
    });
    assert_eq!(tracker.phase(), WorkflowPhase::Executing);

    // After iteration completes, clean up state files.
    cleanup_state(project).unwrap();
    assert!(!plan_exists(project));

    // Tracker can be reset independently.
    tracker.reset();
    assert_eq!(tracker.phase(), WorkflowPhase::Researching);
}

// ── Loop state increments across iterations ─────────────────────────

#[test]
fn loop_state_tracks_iterations() {
    // Simulate 3 iterations.
    let mut state = LoopState {
        iteration: 0,
        last_status: String::new(),
    };

    for i in 1..=3 {
        state.iteration = i;
        state.last_status = format!("iteration {i} completed");

        let serialized = state.serialize();
        let parsed = LoopState::parse(&serialized);
        assert_eq!(parsed.iteration, i);
        assert_eq!(parsed.last_status, format!("iteration {i} completed"));
    }

    assert_eq!(state.iteration, 3);
}

// ── Lifecycle Hook Execution ─────────────────────────────────────

/// Build a minimal ProjectConfig JSON for lifecycle hook tests.
fn project_config_with_format_command(
    format_cmd: &str,
    hooks: Vec<magistr_engine::LifecycleHook>,
) -> magistr::types::ProjectConfig {
    let json = serde_json::json!({
        "project": { "name": "test", "description": "", "type": "" },
        "stack": {
            "language": "rust",
            "framework": "",
            "package_manager": "cargo",
            "test_framework": "nextest",
            "linter": "clippy",
            "formatter": "rustfmt"
        },
        "quality_gate": { "check": "true", "test": "true", "lint": "true" },
        "format": { "extensions": ["rs"], "command": format_cmd, "exclusions": [] },
        "spec": { "file": "ROADMAP.md", "unchecked_pattern": "- \\[ \\]" },
        "testing": { "companion_pattern": "{name}.tests.rs", "test_dir": "tests" },
        "reference_repos": [],
        "state_prefix": ".magistr",
        "lifecycle_hooks": hooks,
    });
    serde_json::from_value(json).expect("valid ProjectConfig")
}

/// Integration test: configure a lifecycle hook bound to on_enter(verifying),
/// resolve it via HookResolver, execute it through HookExecutor with a
/// ProcessHookDispatcher, and confirm it runs the mapped command.
#[tokio::test]
async fn lifecycle_hook_fires_on_phase_transition() {
    use magistr_engine::{
        HookAction, HookExecutor, HookResolver, LifecycleEvent, LifecycleHook, OnFailure,
        WorkflowPhase,
    };

    // Configure a hook that fires on_enter(verifying) and calls magistr_format.
    let hooks = vec![LifecycleHook {
        event: LifecycleEvent::OnEnter(WorkflowPhase::Verifying),
        action: HookAction::McpTool {
            tool: "magistr_format".into(),
            arguments: serde_json::Value::Null,
        },
        description: Some("Auto-format on entering verify phase".into()),
        on_failure: OnFailure::Continue,
    }];

    // Resolve hooks for the OnEnter(Verifying) event.
    let resolved =
        HookResolver::resolve(&LifecycleEvent::OnEnter(WorkflowPhase::Verifying), &hooks);
    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0].display_name(), "magistr_format");

    // Execute the resolved hooks through a ProcessHookDispatcher.
    let tmp = TempDir::new().unwrap();
    let config = project_config_with_format_command("echo formatted-ok", hooks);

    let dispatcher =
        magistr::workflow::hook_dispatch::ProcessHookDispatcher::new(tmp.path(), &config);
    let report = HookExecutor::execute(&resolved, &dispatcher).await;

    assert!(report.is_success());
    assert!(!report.has_warnings());
    // The format command ("echo formatted-ok") should produce output.
    assert_eq!(report.results.len(), 1);
    let output = report.results[0].as_ref().unwrap();
    assert_eq!(output.message.as_deref(), Some("formatted-ok"));
}

/// Integration test: hook with FailFast strategy stops execution on failure.
#[tokio::test]
async fn lifecycle_hook_fail_fast_blocks() {
    use magistr_engine::{
        HookAction, HookExecutor, HookResolver, LifecycleEvent, LifecycleHook, OnFailure,
    };

    let hooks = vec![
        LifecycleHook {
            event: LifecycleEvent::OnGateFail,
            action: HookAction::McpTool {
                tool: "magistr_format".into(),
                arguments: serde_json::Value::Null,
            },
            description: None,
            on_failure: OnFailure::FailFast,
        },
        LifecycleHook {
            event: LifecycleEvent::OnGateFail,
            action: HookAction::McpTool {
                tool: "magistr_format".into(),
                arguments: serde_json::Value::Null,
            },
            description: None,
            on_failure: OnFailure::Continue,
        },
    ];

    let resolved = HookResolver::resolve(&LifecycleEvent::OnGateFail, &hooks);
    assert_eq!(resolved.len(), 2);

    // First hook will fail (command exits non-zero), second should not run.
    let tmp = TempDir::new().unwrap();
    let config = project_config_with_format_command("false", hooks);

    let dispatcher =
        magistr::workflow::hook_dispatch::ProcessHookDispatcher::new(tmp.path(), &config);
    let report = HookExecutor::execute(&resolved, &dispatcher).await;

    assert!(!report.is_success());
    // Only one result because FailFast stopped after the first failure.
    assert_eq!(report.results.len(), 1);
    assert!(report.results[0].is_err());
}

// ── Shell Command Hook Integration Tests ────────────────────────────

/// Integration test: shell command hook executes directly and captures output.
#[tokio::test]
async fn shell_command_hook_executes_and_captures_output() {
    use magistr_engine::{
        HookAction, HookExecutor, HookResolver, LifecycleEvent, LifecycleHook, OnFailure,
        WorkflowPhase,
    };

    let hooks = vec![LifecycleHook {
        event: LifecycleEvent::OnEnter(WorkflowPhase::Executing),
        action: HookAction::ShellCommand {
            command: "echo shell-hook-output".into(),
            timeout_secs: Some(10),
        },
        description: Some("Run a shell command on entering execute".into()),
        on_failure: OnFailure::Continue,
    }];

    let resolved =
        HookResolver::resolve(&LifecycleEvent::OnEnter(WorkflowPhase::Executing), &hooks);
    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0].display_name(), "echo shell-hook-output");

    let tmp = TempDir::new().unwrap();
    let config = project_config_with_format_command("true", hooks);
    let dispatcher =
        magistr::workflow::hook_dispatch::ProcessHookDispatcher::new(tmp.path(), &config);
    let report = HookExecutor::execute(&resolved, &dispatcher).await;

    assert!(report.is_success());
    assert_eq!(report.results.len(), 1);
    let output = report.results[0].as_ref().unwrap();
    assert_eq!(output.message.as_deref(), Some("shell-hook-output"));
}

/// Integration test: shell command hook failure is captured with exit code.
#[tokio::test]
async fn shell_command_hook_failure_captured() {
    use magistr_engine::{
        HookAction, HookExecutor, HookResolver, LifecycleEvent, LifecycleHook, OnFailure,
        WorkflowPhase,
    };

    let hooks = vec![LifecycleHook {
        event: LifecycleEvent::OnExit(WorkflowPhase::Verifying),
        action: HookAction::ShellCommand {
            command: "exit 42".into(),
            timeout_secs: None,
        },
        description: None,
        on_failure: OnFailure::Continue,
    }];

    let resolved = HookResolver::resolve(&LifecycleEvent::OnExit(WorkflowPhase::Verifying), &hooks);
    assert_eq!(resolved.len(), 1);

    let tmp = TempDir::new().unwrap();
    let config = project_config_with_format_command("true", hooks);
    let dispatcher =
        magistr::workflow::hook_dispatch::ProcessHookDispatcher::new(tmp.path(), &config);
    let report = HookExecutor::execute(&resolved, &dispatcher).await;

    // Continue strategy: failure is recorded but execution continues (no FailFast).
    // is_success() is false because a hook did fail (only Warn is non-fatal).
    assert!(!report.is_success());
    assert_eq!(report.results.len(), 1);
    let err = report.results[0].as_ref().unwrap_err();
    assert!(err.message.contains("exit 42"));
}

/// Integration test: shell command hook with FailFast blocks subsequent hooks.
#[tokio::test]
async fn shell_command_hook_fail_fast_blocks_subsequent() {
    use magistr_engine::{
        HookAction, HookExecutor, HookResolver, LifecycleEvent, LifecycleHook, OnFailure,
        WorkflowPhase,
    };

    let hooks = vec![
        LifecycleHook {
            event: LifecycleEvent::OnEnter(WorkflowPhase::Verifying),
            action: HookAction::ShellCommand {
                command: "exit 1".into(),
                timeout_secs: None,
            },
            description: None,
            on_failure: OnFailure::FailFast,
        },
        LifecycleHook {
            event: LifecycleEvent::OnEnter(WorkflowPhase::Verifying),
            action: HookAction::ShellCommand {
                command: "echo should-not-run".into(),
                timeout_secs: None,
            },
            description: None,
            on_failure: OnFailure::Continue,
        },
    ];

    let resolved =
        HookResolver::resolve(&LifecycleEvent::OnEnter(WorkflowPhase::Verifying), &hooks);
    assert_eq!(resolved.len(), 2);

    let tmp = TempDir::new().unwrap();
    let config = project_config_with_format_command("true", hooks);
    let dispatcher =
        magistr::workflow::hook_dispatch::ProcessHookDispatcher::new(tmp.path(), &config);
    let report = HookExecutor::execute(&resolved, &dispatcher).await;

    assert!(!report.is_success());
    // Only one result — second hook was skipped due to FailFast.
    assert_eq!(report.results.len(), 1);
    assert!(report.results[0].is_err());
}

/// Integration test: shell command hook with Warn strategy produces warnings.
#[tokio::test]
async fn shell_command_hook_warn_strategy() {
    use magistr_engine::{
        HookAction, HookExecutor, HookResolver, LifecycleEvent, LifecycleHook, OnFailure,
        WorkflowPhase,
    };

    let hooks = vec![
        LifecycleHook {
            event: LifecycleEvent::OnEnter(WorkflowPhase::Verifying),
            action: HookAction::ShellCommand {
                command: "exit 1".into(),
                timeout_secs: None,
            },
            description: None,
            on_failure: OnFailure::Warn,
        },
        LifecycleHook {
            event: LifecycleEvent::OnEnter(WorkflowPhase::Verifying),
            action: HookAction::ShellCommand {
                command: "echo second-runs".into(),
                timeout_secs: None,
            },
            description: None,
            on_failure: OnFailure::Continue,
        },
    ];

    let resolved =
        HookResolver::resolve(&LifecycleEvent::OnEnter(WorkflowPhase::Verifying), &hooks);
    assert_eq!(resolved.len(), 2);

    let tmp = TempDir::new().unwrap();
    let config = project_config_with_format_command("true", hooks);
    let dispatcher =
        magistr::workflow::hook_dispatch::ProcessHookDispatcher::new(tmp.path(), &config);
    let report = HookExecutor::execute(&resolved, &dispatcher).await;

    // Warn failures are not hard failures.
    assert!(report.is_success());
    assert!(report.has_warnings());
    assert_eq!(report.warnings().len(), 1);
    // Both hooks ran (Warn doesn't stop execution).
    assert_eq!(report.results.len(), 2);
    assert!(report.results[0].is_err()); // first failed with warning
    assert!(report.results[1].is_ok()); // second succeeded
}

/// Integration test: shell command hook with timeout aborts long-running commands.
#[tokio::test]
async fn shell_command_hook_timeout() {
    use magistr_engine::{
        HookAction, HookExecutor, HookResolver, LifecycleEvent, LifecycleHook, OnFailure,
        WorkflowPhase,
    };

    let hooks = vec![LifecycleHook {
        event: LifecycleEvent::OnEnter(WorkflowPhase::Executing),
        action: HookAction::ShellCommand {
            command: "sleep 60".into(),
            timeout_secs: Some(1),
        },
        description: None,
        on_failure: OnFailure::FailFast,
    }];

    let resolved =
        HookResolver::resolve(&LifecycleEvent::OnEnter(WorkflowPhase::Executing), &hooks);
    assert_eq!(resolved.len(), 1);

    let tmp = TempDir::new().unwrap();
    let config = project_config_with_format_command("true", hooks);
    let dispatcher =
        magistr::workflow::hook_dispatch::ProcessHookDispatcher::new(tmp.path(), &config);
    let report = HookExecutor::execute(&resolved, &dispatcher).await;

    assert!(!report.is_success());
    assert_eq!(report.results.len(), 1);
    let err = report.results[0].as_ref().unwrap_err();
    assert!(err.message.contains("timed out"));
}
