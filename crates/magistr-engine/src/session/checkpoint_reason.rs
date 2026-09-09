use std::fmt;

use serde::{Deserialize, Serialize};

/// What put a checkpoint on the spine.
///
/// The reason is on the record because the three are not
/// interchangeable to a reader deciding where to go back to: an
/// iteration boundary is a state the loop itself thought was worth
/// having, a risk checkpoint is the state immediately *before*
/// something that might not be undoable, and a manual one is where a
/// human said "here".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckpointReason {
    /// The loop reached an iteration boundary.
    Iteration,
    /// A high-risk permission was about to be answered.
    Risk,
    /// The human asked for one.
    Manual,
}

impl CheckpointReason {
    /// The reason in one word, as the rail prints it.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Iteration => "iteration",
            Self::Risk => "risk",
            Self::Manual => "manual",
        }
    }
}

impl fmt::Display for CheckpointReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}
