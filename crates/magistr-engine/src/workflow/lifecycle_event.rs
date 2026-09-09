use std::fmt;

use serde::{Deserialize, Serialize};

use super::workflow_phase::WorkflowPhase;

/// Events in the workflow lifecycle that can trigger hooks.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", content = "phase", rename_all = "snake_case")]
pub enum LifecycleEvent {
    /// Fired when entering a workflow phase.
    OnEnter(WorkflowPhase),
    /// Fired when exiting a workflow phase.
    OnExit(WorkflowPhase),
    /// Fired at the start of a new iteration.
    OnIterationStart,
    /// Fired at the end of an iteration.
    OnIterationEnd,
    /// Fired when all quality gates pass.
    OnGatePass,
    /// Fired when a quality gate fails.
    OnGateFail,
}

impl fmt::Display for LifecycleEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OnEnter(phase) => write!(f, "on_enter({phase})"),
            Self::OnExit(phase) => write!(f, "on_exit({phase})"),
            Self::OnIterationStart => write!(f, "on_iteration_start"),
            Self::OnIterationEnd => write!(f, "on_iteration_end"),
            Self::OnGatePass => write!(f, "on_gate_pass"),
            Self::OnGateFail => write!(f, "on_gate_fail"),
        }
    }
}
