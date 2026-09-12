//! Why a check could not be admitted, observed or settled.

use std::fmt;

use magistr_workspace::snapshot::SnapshotError;

/// A typed refusal from the composition boundary. Each variant names the
/// authority that refused: the workflow's task admission, the workspace's
/// source observation, or the workflow's settlement. None of them is a
/// degraded answer — an admitted check either settles or reports why not.
#[derive(Debug)]
pub enum CheckEvidenceError {
    /// The workflow could not name a task to admit the check against, or
    /// refused the request itself (no active task, an ambiguous id, no
    /// execution identity).
    Admission(String),
    /// The workspace could not be observed, before or after the run.
    Source(SnapshotError),
    /// The workflow refused to settle the admitted check.
    Settlement(String),
}

impl fmt::Display for CheckEvidenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Admission(reason) => write!(f, "check admission refused: {reason}"),
            Self::Source(error) => write!(f, "source observation refused: {error}"),
            Self::Settlement(reason) => write!(f, "check settlement refused: {reason}"),
        }
    }
}

impl std::error::Error for CheckEvidenceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::Admission(_) | Self::Settlement(_) => None,
        }
    }
}

impl From<SnapshotError> for CheckEvidenceError {
    fn from(error: SnapshotError) -> Self {
        Self::Source(error)
    }
}
