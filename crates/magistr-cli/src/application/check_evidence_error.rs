//! Why a check could not be admitted, observed or settled.

use magistr_workflow::ship::application::ShipError;
use std::fmt;

/// A typed refusal from the composition boundary. Each variant names the
/// authority that refused: the workflow's task admission, the workspace's
/// source observation (surfaced by the workflow's evidence capability), or
/// the workflow's settlement. None of them is a degraded answer — an
/// admitted check either settles or reports why not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckEvidenceError {
    /// The workflow could not name a task to admit the check against, or
    /// refused the request itself (no active task, an ambiguous id, no
    /// execution identity, no objective).
    Admission(ShipError),
    /// The workspace could not be observed at admission.
    Source(ShipError),
    /// The workflow refused to settle the admitted check.
    Settlement(ShipError),
}

impl CheckEvidenceError {
    /// Preserve the workflow owner's refusal or uncertainty at the tool boundary.
    pub(crate) fn mutation_failure(&self) -> magistr_engine::native::WorkflowMutationError {
        match self {
            Self::Admission(reason) | Self::Source(reason) | Self::Settlement(reason) => {
                reason.clone().into()
            }
        }
    }

    /// Classify the workflow's typed admission failure without parsing copy.
    pub(super) fn from_admission(reason: ShipError) -> Self {
        if reason.kind() == "source_unavailable" {
            Self::Source(reason)
        } else {
            Self::Admission(reason)
        }
    }
}

impl fmt::Display for CheckEvidenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Admission(reason) => write!(f, "check admission refused: {reason}"),
            Self::Source(reason) => write!(f, "source observation refused: {reason}"),
            Self::Settlement(reason) => write!(f, "check settlement refused: {reason}"),
        }
    }
}

impl std::error::Error for CheckEvidenceError {}
