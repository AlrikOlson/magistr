use serde::Serialize;

use crate::run::domain::step::{Step, StepStatus, StepType};

/// Typed plan view; its serialization preserves the MCP plan response.
#[derive(Debug, Clone, Serialize)]
pub struct PlanSnapshot {
    steps: Vec<PlanStep>,
    total: usize,
}

impl PlanSnapshot {
    pub(crate) fn new(steps: &[Step]) -> Self {
        Self {
            steps: steps
                .iter()
                .map(|step| PlanStep {
                    id: step.id.clone(),
                    title: step.title.clone(),
                    step_type: step.step_type.clone(),
                    status: step.status,
                    estimate: step.estimate.clone(),
                })
                .collect(),
            total: steps.len(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct PlanStep {
    id: String,
    title: String,
    #[serde(rename = "type")]
    step_type: StepType,
    status: StepStatus,
    estimate: Option<String>,
}
