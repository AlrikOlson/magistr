//! Convert owner failures without dropping the mutation's effect or cause.

use magistr_workflow_contracts::WorkflowMutationError;

impl From<crate::plan::application::PlanError> for WorkflowMutationError {
    fn from(error: crate::plan::application::PlanError) -> Self {
        Self {
            code: error.kind().into(),
            message: error.to_string(),
            effect: error.effect().into(),
            store: error.store_error().cloned().map(Box::new),
            record_id: None,
            committed_record: None,
        }
    }
}

impl From<crate::request::application::RequestError> for WorkflowMutationError {
    fn from(error: crate::request::application::RequestError) -> Self {
        let committed_record = match &error {
            crate::request::application::RequestError::PartialPromotion {
                task_id,
                committed_source,
                ..
            } => Some(Box::new(magistr_workflow_contracts::Observed {
                value: task_id.clone(),
                source: *committed_source,
            })),
            _ => None,
        };
        Self {
            code: error.kind().into(),
            message: error.to_string(),
            effect: error.effect().into(),
            store: error.store_error().cloned().map(Box::new),
            record_id: None,
            committed_record,
        }
    }
}

impl From<crate::run::application::RunError> for WorkflowMutationError {
    fn from(error: crate::run::application::RunError) -> Self {
        Self {
            code: error.kind().into(),
            message: error.to_string(),
            effect: error.effect().into(),
            store: error.store_error().cloned().map(Box::new),
            record_id: None,
            committed_record: None,
        }
    }
}
