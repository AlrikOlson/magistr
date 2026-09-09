use alloc::string::String;
use core::fmt;
use magistr_domain::identity::{AttemptId, ProjectId, RunId, SessionId};

use crate::{Capability, RecoveryAction, Revision};

/// Machine-readable failure and recovery evidence. Adapters preserve uncertainty:
/// after a possibly applied effect, use UnknownOutcome rather than inventing a
/// clean persistence failure or disconnect that could encourage duplicate work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplicationError {
    ProjectMismatch {
        expected: ProjectId,
        actual: ProjectId,
    },
    ScopeMismatch {
        expected: RunId,
        actual: RunId,
    },
    InvalidRequest {
        reason: String,
    },
    InvalidResponse {
        reason: String,
    },
    RevisionConflict {
        expected: Revision,
        actual: Revision,
    },
    CapabilityDenied {
        capability: Capability,
        reason: String,
    },
    CapabilityUnavailable {
        capability: Capability,
    },
    PersistenceFailure {
        operation: String,
    },
    ProviderDisconnected {
        session: SessionId,
    },
    UnknownOutcome {
        attempt: AttemptId,
    },
}

impl ApplicationError {
    #[must_use]
    pub fn recovery(&self) -> RecoveryAction {
        match self {
            Self::ProjectMismatch { .. }
            | Self::ScopeMismatch { .. }
            | Self::InvalidRequest { .. } => RecoveryAction::CorrectRequest,
            Self::RevisionConflict { .. } => RecoveryAction::RefreshRevision,
            Self::CapabilityDenied { .. } => RecoveryAction::ReviewAuthorization,
            Self::CapabilityUnavailable { .. } => RecoveryAction::SelectAvailableCapability,
            Self::PersistenceFailure { .. } => RecoveryAction::RestorePersistence,
            Self::ProviderDisconnected { .. } => RecoveryAction::ReconnectProvider,
            Self::InvalidResponse { .. } | Self::UnknownOutcome { .. } => {
                RecoveryAction::ReconcileOutcome
            }
        }
    }
}

impl fmt::Display for ApplicationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProjectMismatch { .. } => f.write_str("request belongs to another project"),
            Self::ScopeMismatch { .. } => f.write_str("target belongs to another run"),
            Self::InvalidRequest { reason } => write!(f, "invalid application request: {reason}"),
            Self::InvalidResponse { reason } => write!(f, "invalid adapter response: {reason}"),
            Self::RevisionConflict { expected, actual } => write!(
                f,
                "revision conflict: expected {}, observed {}",
                expected.0, actual.0
            ),
            Self::CapabilityDenied { capability, reason } => {
                write!(f, "{capability:?} denied: {reason}")
            }
            Self::CapabilityUnavailable { capability } => write!(f, "{capability:?} unavailable"),
            Self::PersistenceFailure { operation } => {
                write!(f, "persistence failed during {operation}")
            }
            Self::ProviderDisconnected { .. } => f.write_str("provider disconnected"),
            Self::UnknownOutcome { .. } => {
                f.write_str("command outcome unknown; reconcile before retrying")
            }
        }
    }
}

impl core::error::Error for ApplicationError {}

#[cfg(test)]
#[path = "application_error.tests.rs"]
mod tests;
