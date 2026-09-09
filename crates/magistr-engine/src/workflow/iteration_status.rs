use serde::{Deserialize, Serialize};
use std::fmt;

/// Status of a completed iteration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IterationStatus {
    /// No iteration has run yet.
    None,
    /// Iteration committed successfully.
    Committed,
    /// Iteration completed but no commit was detected.
    NoCommit,
    /// Iteration failed (agent error).
    Failed,
}

impl fmt::Display for IterationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => f.write_str("none"),
            Self::Committed => f.write_str("committed"),
            Self::NoCommit => f.write_str("no_commit"),
            Self::Failed => f.write_str("failed"),
        }
    }
}
