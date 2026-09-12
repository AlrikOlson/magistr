//! Why a check could not be admitted, observed or settled.

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
    Admission(String),
    /// The workspace could not be observed at admission.
    Source(String),
    /// The workflow refused to settle the admitted check.
    Settlement(String),
}

impl CheckEvidenceError {
    /// The workflow's admission refusal, told apart by its own prefix when
    /// the evidence capability was what refused.
    pub(super) fn from_admission(reason: String) -> Self {
        match reason.strip_prefix("source observation refused: ") {
            Some(source) => Self::Source(source.to_owned()),
            None => Self::Admission(reason),
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
