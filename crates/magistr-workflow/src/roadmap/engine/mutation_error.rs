use crate::infra::transaction::TransactionError;
use magistr_workflow_contracts::WriteError;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RoadmapMutationError {
    #[error("{0}")]
    Rejected(String),
    #[error("{0}")]
    Store(WriteError),
}

impl From<TransactionError<String>> for RoadmapMutationError {
    fn from(error: TransactionError<String>) -> Self {
        match error {
            TransactionError::Rejected(message) => Self::Rejected(message),
            TransactionError::Store(error) => Self::Store(error),
        }
    }
}
