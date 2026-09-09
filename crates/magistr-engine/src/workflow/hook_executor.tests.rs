use super::*;
use crate::workflow::lifecycle_hook::{HookAction, OnFailure, ResolvedHook};
use serde_json::json;
use std::sync::{Arc, Mutex};

/// Mock dispatcher that records calls and returns configurable results.
#[derive(Clone)]
struct MockDispatcher {
    calls: Arc<Mutex<Vec<HookAction>>>,
    results: Arc<Mutex<Vec<Result<HookOutput, HookError>>>>,
}

impl MockDispatcher {
    fn new(results: Vec<Result<HookOutput, HookError>>) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            results: Arc::new(Mutex::new(results)),
        }
    }

    fn succeeding(count: usize) -> Self {
        let results = (0..count)
            .map(|_| Ok(HookOutput { message: None }))
            .collect();
        Self::new(results)
    }

    fn calls(&self) -> Vec<HookAction> {
        self.calls.lock().unwrap().clone()
    }
}

impl HookDispatcher for MockDispatcher {
    async fn dispatch(&self, action: &HookAction) -> Result<HookOutput, HookError> {
        self.calls.lock().unwrap().push(action.clone());
        self.results.lock().unwrap().remove(0)
    }
}

fn resolved(tool: &str, on_failure: OnFailure) -> ResolvedHook {
    ResolvedHook {
        action: HookAction::McpTool {
            tool: tool.to_owned(),
            arguments: json!({}),
        },
        description: None,
        on_failure,
    }
}

#[tokio::test]
async fn empty_hooks_returns_empty_report() {
    let dispatcher = MockDispatcher::succeeding(0);
    let report = HookExecutor::execute(&[], &dispatcher).await;
    assert!(report.results.is_empty());
    assert!(report.is_success());
    assert!(!report.has_warnings());
}

#[tokio::test]
async fn executes_hooks_sequentially() {
    let hooks = vec![
        resolved("tool_a", OnFailure::Continue),
        resolved("tool_b", OnFailure::Continue),
        resolved("tool_c", OnFailure::Continue),
    ];
    let dispatcher = MockDispatcher::succeeding(3);
    let report = HookExecutor::execute(&hooks, &dispatcher).await;

    assert_eq!(report.results.len(), 3);
    assert!(report.is_success());

    let calls = dispatcher.calls();
    assert_eq!(calls.len(), 3);
    assert_eq!(calls[0].display_name(), "tool_a");
    assert_eq!(calls[1].display_name(), "tool_b");
    assert_eq!(calls[2].display_name(), "tool_c");
}

#[tokio::test]
async fn fail_fast_stops_on_first_failure() {
    let hooks = vec![
        resolved("tool_a", OnFailure::FailFast),
        resolved("tool_b", OnFailure::FailFast),
    ];
    let dispatcher = MockDispatcher::new(vec![
        Err(HookError {
            message: "tool_a failed".into(),
        }),
        Ok(HookOutput { message: None }),
    ]);
    let report = HookExecutor::execute(&hooks, &dispatcher).await;

    assert!(!report.is_success());
    // Only one hook executed — fail-fast stopped after the first failure
    assert_eq!(report.results.len(), 1);
    assert!(report.results[0].is_err());
    assert_eq!(dispatcher.calls().len(), 1);
}

#[tokio::test]
async fn continue_strategy_runs_all_hooks() {
    let hooks = vec![
        resolved("tool_a", OnFailure::Continue),
        resolved("tool_b", OnFailure::Continue),
    ];
    let dispatcher = MockDispatcher::new(vec![
        Err(HookError {
            message: "tool_a failed".into(),
        }),
        Ok(HookOutput { message: None }),
    ]);
    let report = HookExecutor::execute(&hooks, &dispatcher).await;

    // Both hooks ran despite first failure
    assert_eq!(report.results.len(), 2);
    assert!(report.results[0].is_err());
    assert!(report.results[1].is_ok());
    assert_eq!(dispatcher.calls().len(), 2);
    // Report is not success (has errors)
    assert!(!report.is_success());
}

#[tokio::test]
async fn warn_strategy_collects_warnings() {
    let hooks = vec![
        resolved("tool_a", OnFailure::Warn),
        resolved("tool_b", OnFailure::Continue),
    ];
    let dispatcher = MockDispatcher::new(vec![
        Err(HookError {
            message: "soft failure".into(),
        }),
        Ok(HookOutput { message: None }),
    ]);
    let report = HookExecutor::execute(&hooks, &dispatcher).await;

    assert_eq!(report.results.len(), 2);
    // Warn failures don't block but are surfaced
    assert!(report.has_warnings());
    assert_eq!(report.warnings().len(), 1);
    assert!(report.warnings()[0].contains("soft failure"));
    // Not a hard failure — is_success is true for warn-only errors
    assert!(report.is_success());
}

#[tokio::test]
async fn passes_arguments_to_dispatcher() {
    let hooks = vec![ResolvedHook {
        action: HookAction::McpTool {
            tool: "magistr_format".to_owned(),
            arguments: json!({"style": "compact"}),
        },
        description: None,
        on_failure: OnFailure::Continue,
    }];
    let dispatcher = MockDispatcher::succeeding(1);
    HookExecutor::execute(&hooks, &dispatcher).await;

    let calls = dispatcher.calls();
    match &calls[0] {
        HookAction::McpTool { arguments, .. } => {
            assert_eq!(arguments, &json!({"style": "compact"}));
        }
        _ => panic!("expected McpTool"),
    }
}

#[tokio::test]
async fn mixed_strategies_fail_fast_after_continue() {
    // Continue hook fails (non-blocking), then FailFast hook fails (blocks)
    let hooks = vec![
        resolved("tool_a", OnFailure::Continue),
        resolved("tool_b", OnFailure::FailFast),
        resolved("tool_c", OnFailure::Continue),
    ];
    let dispatcher = MockDispatcher::new(vec![
        Err(HookError {
            message: "ignored".into(),
        }),
        Err(HookError {
            message: "fatal".into(),
        }),
        Ok(HookOutput { message: None }),
    ]);
    let report = HookExecutor::execute(&hooks, &dispatcher).await;

    // tool_a ran (Continue), tool_b ran (FailFast, stopped), tool_c skipped
    assert_eq!(report.results.len(), 2);
    assert_eq!(dispatcher.calls().len(), 2);
    assert!(!report.is_success());
}

#[tokio::test]
async fn shell_command_hooks_dispatch_correctly() {
    let hooks = vec![ResolvedHook {
        action: HookAction::ShellCommand {
            command: "echo test".to_owned(),
            timeout_secs: Some(10),
        },
        description: None,
        on_failure: OnFailure::Continue,
    }];
    let dispatcher = MockDispatcher::succeeding(1);
    let report = HookExecutor::execute(&hooks, &dispatcher).await;

    assert!(report.is_success());
    let calls = dispatcher.calls();
    assert!(calls[0].is_shell_command());
    assert_eq!(calls[0].display_name(), "echo test");
}
