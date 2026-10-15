//! An unfinished or closed execution retained while another objective is selected.

use super::domain::{execution_identity::ExecutionIdentity, objective::Objective, task::Task};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SavedCycle {
    pub objective: Objective,
    pub tasks: Vec<Task>,
    pub next_action_id: u32,
    pub execution: ExecutionIdentity,
}
