use crate::run::domain::step::StepType;

/// A plan mutation before execution; defaults and refusals belong to the use case.
#[derive(Debug, Clone)]
pub enum PlanChange {
    Add {
        id: String,
        title: Option<String>,
        step_type: Option<StepType>,
        estimate: Option<String>,
        think_branch: Option<String>,
    },
    Remove {
        id: String,
    },
    Reorder {
        id: String,
        after: Option<String>,
    },
    Unknown,
}
