//! Workflow tracking state (domain-only).
//!
//! Core workflow phase tracking, gate results, and iteration history.
//! No engine hook resolution, no socket IPC, no tip engine, no modals —
//! those are frontend-specific concerns.

use super::gate::{GATE_HISTORY_MAX, GateResult};
use super::loop_progress::LoopProgress;
use super::workflow_phase::WorkflowPhase;

/// Domain-only workflow state.
///
/// Frontend implementations extend this with presentation-specific fields
/// (lifecycle hooks, socket IPC, tip engine, gate detail modals, pause UI).
#[derive(Debug, Clone, Default)]
pub struct WorkflowState {
    /// Current workflow phase (Research/Plan/Execute/Verify).
    pub workflow_phase: Option<WorkflowPhase>,
    /// Whether `.magistr/current-plan.md` has been written during this session.
    pub plan_file_written: bool,
    /// Quality gate results (populated after agent turn in workflow mode).
    pub gate_results: Vec<GateResult>,
    /// Gate results history across iterations (last N runs) for sparkline display.
    pub gate_results_history: Vec<Vec<GateResult>>,
    /// Count of consecutive gate failures (reset on success).
    pub consecutive_failures: u32,
    /// Loop progress state (active during next-loop).
    pub loop_progress: Option<LoopProgress>,
    /// Accumulated time per workflow phase label (seconds).
    pub phase_durations: Vec<(String, f64)>,
}

impl WorkflowState {
    /// Transition the workflow phase.
    ///
    /// Returns `Some(message)` when a phase transition occurs, `None` for initial set.
    pub fn set_workflow_phase(&mut self, phase: WorkflowPhase) -> Option<String> {
        let prev = self.workflow_phase;
        self.workflow_phase = Some(phase);

        if prev.is_some() && prev != Some(phase) {
            Some(format!("Phase: {}", phase.label()))
        } else {
            None
        }
    }

    /// Snapshot the current gate results into history.
    ///
    /// Keeps at most [`GATE_HISTORY_MAX`] entries. Oldest entries are dropped.
    pub fn push_gate_history(&mut self) {
        if self.gate_results.is_empty() {
            return;
        }
        if self.gate_results_history.len() >= GATE_HISTORY_MAX {
            self.gate_results_history.remove(0);
        }
        self.gate_results_history.push(self.gate_results.clone());
    }
}

#[cfg(test)]
#[path = "workflow_state.tests.rs"]
mod tests;
