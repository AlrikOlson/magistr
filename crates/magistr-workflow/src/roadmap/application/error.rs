use crate::roadmap::engine::RoadmapMutationError;
use magistr_workflow_contracts::WriteError;

/// A native roadmap failure with stable client-facing classification.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RoadmapError {
    #[error("{0}")]
    InvalidArgs(String),
    #[error("{0}")]
    InvalidState(String),
    #[error(
        "file export requires a configured workspace; use output: inline to read without writing"
    )]
    WorkspaceUnavailable,
    #[error("{0}")]
    ExportFailed(String),
    #[error("roadmap engine mutex poisoned")]
    Poisoned,
    #[error("{0}")]
    Store(WriteError),
}

impl RoadmapError {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::InvalidArgs(_) => "invalid_args",
            Self::InvalidState(_) => "invalid_state",
            Self::WorkspaceUnavailable => "workspace_unavailable",
            Self::ExportFailed(_) => "export_failed",
            Self::Poisoned => "internal_error",
            Self::Store(_) => "store_unavailable",
        }
    }

    pub fn store_error(&self) -> Option<&WriteError> {
        match self {
            Self::Store(error) => Some(error),
            _ => None,
        }
    }

    pub fn effect(&self) -> &'static str {
        match self {
            Self::Store(error) => error.effect(),
            _ => "none",
        }
    }

    pub(crate) fn invalid_args(error: RoadmapMutationError) -> Self {
        match error {
            RoadmapMutationError::Rejected(message) => Self::InvalidArgs(message),
            RoadmapMutationError::Store(error) => Self::Store(error),
        }
    }

    pub(crate) fn invalid_state(error: RoadmapMutationError) -> Self {
        match error {
            RoadmapMutationError::Rejected(message) => Self::InvalidState(message),
            RoadmapMutationError::Store(error) => Self::Store(error),
        }
    }
}
