use crate::request::engine::RequestMutationError;
use magistr_workflow_contracts::WriteError;

/// A request use-case failure, independent of transport error envelopes.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RequestError {
    #[error("{0}")]
    InvalidArgs(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    InvalidState(String),
    #[error("request_promote requires the plan engine (not wired in this context)")]
    NotWired,
    #[error("request engine mutex poisoned")]
    Poisoned,
    #[error("{0}")]
    Store(WriteError),
    #[error(
        "Work item '{task_id}' was saved, but promotion could not finish: {cause}. Read both records and retry promotion."
    )]
    PartialPromotion {
        task_id: String,
        committed_source: magistr_workflow_contracts::ReadSource,
        cause: Box<RequestError>,
    },
}

impl RequestError {
    /// Stable logical classification used by existing clients.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::InvalidArgs(_) => "invalid_args",
            Self::NotFound(_) => "not_found",
            Self::InvalidState(_) => "invalid_state",
            Self::NotWired => "not_wired",
            Self::Poisoned => "internal_error",
            Self::Store(_) => "store_unavailable",
            Self::PartialPromotion { .. } => "promotion_incomplete",
        }
    }

    pub fn store_error(&self) -> Option<&WriteError> {
        match self {
            Self::Store(error) => Some(error),
            Self::PartialPromotion { cause, .. } => cause.store_error(),
            _ => None,
        }
    }

    pub fn effect(&self) -> &'static str {
        match self {
            Self::Store(error) => error.effect(),
            Self::PartialPromotion { .. } => "unknown",
            _ => "none",
        }
    }

    pub(crate) fn invalid_args(error: RequestMutationError) -> Self {
        match error {
            RequestMutationError::Rejected(message) => Self::InvalidArgs(message),
            RequestMutationError::Store(error) => Self::Store(error),
        }
    }

    pub(crate) fn invalid_state(error: RequestMutationError) -> Self {
        match error {
            RequestMutationError::Rejected(message) => Self::InvalidState(message),
            RequestMutationError::Store(error) => Self::Store(error),
        }
    }
}
