use serde::{Deserialize, Serialize};

use super::iteration_status::IterationStatus;

/// Tracks progress across iterations in loop mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IterationState {
    /// Current iteration number (0-based).
    pub iteration: u32,
    /// Maximum iterations allowed.
    pub max_iterations: u32,
    /// Status of the last completed iteration.
    pub last_status: IterationStatus,
    /// Spec progress: checked items.
    pub spec_checked: u32,
    /// Spec progress: total items.
    pub spec_total: u32,
}

impl IterationState {
    /// Whether every item in a nonempty spec has been checked.
    #[must_use]
    pub fn is_spec_complete(&self) -> bool {
        self.spec_total > 0 && self.spec_checked >= self.spec_total
    }

    /// Whether the iteration ceiling has been reached or exceeded.
    #[must_use]
    pub fn is_iteration_limit_reached(&self) -> bool {
        self.iteration >= self.max_iterations
    }
}

#[cfg(test)]
#[path = "iteration_state.tests.rs"]
mod tests;
