//! Lifecycle hook presets per language stack.
//!
//! Each supported language gets a sensible default set of hooks that fire
//! at the right workflow phases. Unknown languages get an empty preset.

use super::lifecycle_event::LifecycleEvent;
use super::lifecycle_hook::{HookAction, LifecycleHook, OnFailure};
use super::workflow_phase::WorkflowPhase;

/// Return the default lifecycle hook preset for a given language.
///
/// Language matching is case-insensitive. Returns an empty vec for
/// unknown languages.
///
/// Presets:
/// - **Rust**: `magistr_format` on_enter(verify) + `magistr_gate` on_exit(verify)
/// - **Node/TypeScript/JavaScript**: `magistr_lint` on_enter(verify)
/// - **Python**: `magistr_format` on_enter(execute)
#[must_use]
pub fn preset_for_language(language: &str) -> Vec<LifecycleHook> {
    match language.to_lowercase().as_str() {
        "rust" => rust_preset(),
        "typescript" | "javascript" | "node" => node_preset(),
        "python" => python_preset(),
        _ => Vec::new(),
    }
}

/// Rust: format before verify, run full gate on exit from verify.
fn rust_preset() -> Vec<LifecycleHook> {
    vec![
        LifecycleHook {
            event: LifecycleEvent::OnEnter(WorkflowPhase::Verifying),
            action: HookAction::McpTool {
                tool: "magistr_format".into(),
                arguments: serde_json::Value::Null,
            },
            description: Some("Auto-format before verification".into()),
            on_failure: OnFailure::Continue,
        },
        LifecycleHook {
            event: LifecycleEvent::OnExit(WorkflowPhase::Verifying),
            action: HookAction::McpTool {
                tool: "magistr_gate".into(),
                arguments: serde_json::Value::Null,
            },
            description: Some("Run quality gate after verification".into()),
            on_failure: OnFailure::FailFast,
        },
    ]
}

/// Node/TypeScript/JavaScript: lint before verify.
fn node_preset() -> Vec<LifecycleHook> {
    vec![LifecycleHook {
        event: LifecycleEvent::OnEnter(WorkflowPhase::Verifying),
        action: HookAction::McpTool {
            tool: "magistr_lint".into(),
            arguments: serde_json::Value::Null,
        },
        description: Some("Run linter before verification".into()),
        on_failure: OnFailure::Continue,
    }]
}

/// Python: format on entering execute phase.
fn python_preset() -> Vec<LifecycleHook> {
    vec![LifecycleHook {
        event: LifecycleEvent::OnEnter(WorkflowPhase::Executing),
        action: HookAction::McpTool {
            tool: "magistr_format".into(),
            arguments: serde_json::Value::Null,
        },
        description: Some("Auto-format before execution".into()),
        on_failure: OnFailure::Continue,
    }]
}

#[cfg(test)]
#[path = "hook_presets.tests.rs"]
mod tests;
