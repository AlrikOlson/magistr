//! Convert owner failures without dropping the mutation's effect or cause.

use magistr_workflow_contracts::WorkflowMutationError;

impl From<crate::roadmap::application::RoadmapError> for WorkflowMutationError {
    fn from(error: crate::roadmap::application::RoadmapError) -> Self {
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

impl From<crate::signal::application::SignalError> for WorkflowMutationError {
    fn from(error: crate::signal::application::SignalError) -> Self {
        let committed_record = match &error {
            crate::signal::application::SignalError::PartialPromotion {
                chunk_id,
                committed_source,
                ..
            } => Some(Box::new(magistr_workflow_contracts::Observed {
                value: chunk_id.clone(),
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

impl From<crate::ship::application::ShipError> for WorkflowMutationError {
    fn from(error: crate::ship::application::ShipError) -> Self {
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
