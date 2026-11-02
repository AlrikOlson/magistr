//! An unfinished or closed execution retained while another goal is selected.

use super::domain::{execution_identity::ExecutionIdentity, goal::Goal, step::Step};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SavedCycle {
    pub goal: Goal,
    pub steps: Vec<Step>,
    pub next_progress_id: u32,
    pub execution: ExecutionIdentity,
}
