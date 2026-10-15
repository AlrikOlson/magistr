//! Integration tests for workflow lifecycle hooks and gate enforcement.

use tempfile::TempDir;

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
