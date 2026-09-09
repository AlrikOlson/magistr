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
