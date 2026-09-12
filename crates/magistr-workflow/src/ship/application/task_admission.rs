use serde::{Deserialize, Serialize};

/// A concrete task instance in one project's execution run. Local task names
/// alone are insufficient: a later run may reuse the same name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskAdmission {
    pub project_id: String,
    pub run_id: String,
    pub task_id: String,
    pub task_instance: String,
}
