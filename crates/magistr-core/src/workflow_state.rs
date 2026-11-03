//! Workflow tracking state (domain-only).
//!
//! Core workflow phase tracking, check results, and iteration history.
//! No engine hook resolution, no socket IPC, no tip engine, no modals —
//! those are frontend-specific concerns.

use super::check::{CHECK_HISTORY_MAX, CheckReport};
use super::loop_progress::LoopProgress;
use super::workflow_phase::WorkflowPhase;

/// Domain-only workflow state.
///
/// Frontend implementations extend this with presentation-specific fields
/// (lifecycle hooks, socket IPC, tip engine, check detail modals, pause UI).
#[derive(Debug, Clone, Default)]
pub struct WorkflowState {
    /// Current workflow phase (Research/Plan/Execute/Verify).
    pub workflow_phase: Option<WorkflowPhase>,
    /// Check results (populated after agent turn in workflow mode).
    pub check_results: Vec<CheckReport>,
    /// Check results history across iterations (last N runs) for sparkline display.
    pub check_results_history: Vec<Vec<CheckReport>>,
    /// Count of consecutive check failures (reset on success).
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
            // Progress punctuation belongs to the notification, not the domain value.
            let suffix = if phase.is_terminal() { "" } else { "..." };
            Some(format!("Phase: {}{suffix}", phase.label()))
        } else {
            None
        }
    }

    /// Snapshot the current check results into history.
    ///
    /// Keeps at most [`CHECK_HISTORY_MAX`] entries. Oldest entries are dropped.
    pub fn push_check_history(&mut self) {
        if self.check_results.is_empty() {
            return;
        }
        if self.check_results_history.len() >= CHECK_HISTORY_MAX {
            self.check_results_history.remove(0);
        }
        self.check_results_history.push(self.check_results.clone());
    }
}

#[cfg(test)]
#[path = "workflow_state.tests.rs"]
mod tests;
