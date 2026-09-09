//! Roadmap phase containing tasks.

use serde::{Deserialize, Serialize};

use super::phase_status::PhaseStatus;
use super::task::Task;

/// A roadmap phase containing tasks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Phase {
    /// Short identifier (e.g., "E1", "E2").
    pub id: String,

    /// Human-readable phase name.
    pub name: String,

    /// Optional tagline (rendered after name in heading).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tagline: Option<String>,

    /// Current phase status.
    #[serde(default)]
    pub status: PhaseStatus,

    /// Problem statement (markdown prose, rendered with **Problem:** prefix).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub problem: String,

    /// Solution description (markdown prose, rendered with **Solution:** prefix).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub solution: String,

    /// Phase IDs this phase depends on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,

    /// Ordered list of tasks in this phase.
    #[serde(default)]
    pub tasks: Vec<Task>,
}

impl Phase {
    /// Recompute this phase's status from its tasks.
    pub fn recompute_status(&mut self) {
        if self.status == PhaseStatus::Blocked {
            return; // Don't override manually-set blocked status
        }
        let total = self.total_tasks();
        if total == 0 {
            return; // No tasks — keep current status
        }
        let checked = self.checked_tasks();
        self.status = if checked == total {
            PhaseStatus::Done
        } else if checked > 0 {
            PhaseStatus::InProgress
        } else {
            PhaseStatus::Pending
        };
    }

    /// Count total tasks (including subtasks).
    #[must_use]
    pub fn total_tasks(&self) -> usize {
        self.tasks.iter().map(|t| t.total_count()).sum()
    }

    /// Count checked tasks (including subtasks).
    #[must_use]
    pub fn checked_tasks(&self) -> usize {
        self.tasks.iter().map(|t| t.checked_count()).sum()
    }
}
