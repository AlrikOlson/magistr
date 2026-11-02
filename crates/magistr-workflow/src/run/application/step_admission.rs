use serde::{Deserialize, Serialize};

/// A concrete step instance in one project's execution run. Local step names
/// alone are insufficient: a later run may reuse the same name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct StepAdmission {
    pub project_id: String,
    pub execution: Option<super::ExecutionBinding>,
    pub run_id: String,
    pub step_id: String,
    pub step_instance: String,
}
