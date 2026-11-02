//! Task item with optional nested subtasks.

use serde::{Deserialize, Serialize};

/// A task item with optional subtasks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Task {
    /// Task description text.
    pub text: String,

    /// Whether this task is complete.
    #[serde(default)]
    pub checked: bool,

    /// Additional detail or description (markdown).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub detail: String,

    /// Nested subtasks.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subtasks: Vec<Task>,
}

impl Task {
    /// Count this task plus all subtasks.
    #[must_use]
    pub fn total_count(&self) -> usize {
        1 + self.subtasks.iter().map(|s| s.total_count()).sum::<usize>()
    }

    /// Count checked tasks (this task + checked subtasks).
    #[must_use]
    pub fn checked_count(&self) -> usize {
        let self_count = if self.checked { 1 } else { 0 };
        self_count
            + self
                .subtasks
                .iter()
                .map(|s| s.checked_count())
                .sum::<usize>()
    }
}
