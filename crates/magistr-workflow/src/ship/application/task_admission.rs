use serde::{Deserialize, Serialize};

/// A concrete task instance in one project's execution run. Local task names
/// alone are insufficient: a later run may reuse the same name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TaskAdmission {
    pub project_id: String,
    pub execution: Option<super::ExecutionBinding>,
    pub run_id: String,
    pub task_id: String,
    pub task_instance: String,
}
