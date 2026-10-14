//! Preserve mutation effect and recovery identity across the workflow interfaces.

use crate::WriteError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowMutationError {
    pub code: String,
    pub message: String,
    pub effect: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store: Option<Box<WriteError>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_id: Option<String>,
    /// A separately owned record known to have committed before a partial failure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub committed_record: Option<Box<crate::Observed<String>>>,
}

impl WorkflowMutationError {
    pub fn rejected(message: impl Into<String>) -> Self {
        Self {
            code: "rejected".into(),
            message: message.into(),
            effect: "none".into(),
            store: None,
            record_id: None,
            committed_record: None,
        }
    }

    pub fn uncertain(message: impl Into<String>) -> Self {
        let message = message.into();
        Self {
            code: "write_uncertain".into(),
            store: Some(Box::new(WriteError::uncertain(message.clone()))),
            message,
            effect: "unknown".into(),
            record_id: None,
            committed_record: None,
        }
    }

    #[must_use]
    pub fn effect(&self) -> &str {
        &self.effect
    }
}

impl From<crate::think::StepError> for WorkflowMutationError {
    fn from(error: crate::think::StepError) -> Self {
        Self {
            code: error.kind().into(),
            effect: error.effect().into(),
            message: error.error,
            store: error.store.map(Box::new),
            record_id: error.record_id,
            committed_record: None,
        }
    }
}

impl std::fmt::Display for WorkflowMutationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for WorkflowMutationError {}

#[cfg(test)]
#[path = "mutation_error.tests.rs"]
mod tests;
