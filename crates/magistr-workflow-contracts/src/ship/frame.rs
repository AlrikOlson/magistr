//! Typed mutation records; emission belongs to the workflow host.

use super::{Action, Check, Objective, Task};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ShipFrame {
    ObjectiveSet { objective: Objective },
    TaskAdded { task_id: String, title: String },
    TaskStarted { task_id: String },
    ActionRecorded { task_id: String, action: Action },
    CheckRecorded { task_id: String, check: Check },
    TaskCompleted { task: Task },
    TaskBlocked { task_id: String, reason: String },
    ObjectiveShipped { warnings: Vec<String> },
    Cleared,
}
