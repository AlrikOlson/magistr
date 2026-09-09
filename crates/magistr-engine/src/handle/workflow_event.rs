use serde::{Deserialize, Serialize};

use crate::workflow::WorkflowPhase;
use crate::workflow::lifecycle_event::LifecycleEvent;
use crate::workflow::lifecycle_hook::ResolvedHook;

/// Workflow-specific events emitted by the engine handle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorkflowEvent {
    /// A new iteration started.
    IterationStarted {
        /// Iteration number (1-based).
        iteration: u32,
    },
    /// Workflow phase transitioned.
    PhaseTransition {
        from: WorkflowPhase,
        to: WorkflowPhase,
    },
    /// Quality gate passed.
    GatePassed {
        /// Gate name (e.g., "check", "test", "lint").
        gate: String,
    },
    /// Quality gate failed.
    GateFailed {
        /// Gate name.
        gate: String,
        /// Error output.
        error: String,
    },
    /// Iteration completed.
    IterationCompleted {
        /// Iteration number (1-based).
        iteration: u32,
        /// Whether a commit was produced.
        committed: bool,
    },
    /// Loop stopped (limit reached, spec complete, or repeated failures).
    LoopStopped {
        /// Reason for stopping.
        reason: String,
    },
    /// Lifecycle hooks triggered by a workflow event.
    ///
    /// Subscribers (e.g., the MCP dispatch layer) should execute these hooks.
    LifecycleHooksTriggered {
        /// The event that triggered the hooks.
        event: LifecycleEvent,
        /// Number of hooks to execute.
        hook_count: usize,
        /// Tool names for logging/display.
        tools: Vec<String>,
        /// Resolved hooks ready for dispatch.
        hooks: Vec<ResolvedHook>,
    },
    /// Result of executing lifecycle hooks.
    LifecycleHookResult {
        /// The event that triggered these hooks.
        event: LifecycleEvent,
        /// Whether all hooks passed (or only produced warnings).
        success: bool,
        /// Warning messages from hooks with `Warn` failure strategy.
        warnings: Vec<String>,
        /// Error message if a `FailFast` hook blocked the transition.
        error: Option<String>,
    },
}
