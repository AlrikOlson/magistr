use serde::{Deserialize, Serialize};

/// Aggregate roadmap progress — a lightweight snapshot of completion state.
///
/// This is the engine's view of roadmap progress. The full roadmap schema
/// (phases, tasks, subtasks) lives in magistr-cli; the engine only needs
/// summary statistics for workflow decisions (e.g., "is spec complete?").
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoadmapProgress {
    /// Total number of tasks (including subtasks).
    pub total_tasks: u32,
    /// Number of checked (completed) tasks.
    pub checked_tasks: u32,
    /// Total number of phases.
    pub total_phases: u32,
    /// Number of completed phases.
    pub completed_phases: u32,
}

impl RoadmapProgress {
    /// Overall completion as a fraction (0.0–1.0).
    #[must_use]
    pub fn completion(&self) -> f64 {
        if self.total_tasks == 0 {
            0.0
        } else {
            f64::from(self.checked_tasks) / f64::from(self.total_tasks)
        }
    }

    /// Whether all tasks are complete.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.total_tasks > 0 && self.checked_tasks >= self.total_tasks
    }

    /// Number of remaining unchecked tasks.
    #[must_use]
    pub fn remaining(&self) -> u32 {
        self.total_tasks.saturating_sub(self.checked_tasks)
    }
}
