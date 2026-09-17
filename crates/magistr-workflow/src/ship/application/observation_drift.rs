use std::fmt;

use serde::{Deserialize, Serialize};

/// Which distinguished identity a later observation disagrees with an
/// earlier one on. Each variant is one identity, never a blend: the
/// workspace, the hashing scheme, the selection policy, the selected
/// source bytes or the applicable external configuration — and `Unbound`
/// for a receipt recorded before an identity was bound to receipts at
/// all. The owner generation, the workflow store stamp and a mutation
/// receipt's revision are not observation identities and never appear
/// here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "drift", rename_all = "snake_case")]
pub enum ObservationDrift {
    /// The later observation is of another canonical root.
    ForeignWorkspace { expected: String, actual: String },
    /// The source hashing scheme changed.
    Scheme { before: String, after: String },
    /// The selection policy (what the snapshot selects) changed.
    SelectionPolicy { before: String, after: String },
    /// The selected bytes changed.
    Source { before: String, after: String },
    /// An external configuration layer changed with the bytes unchanged.
    Configuration { before: String, after: String },
    /// The earlier observation predates this identity's binding.
    Unbound { identity: String },
}

fn short(digest: &str) -> &str {
    &digest[..12.min(digest.len())]
}

impl fmt::Display for ObservationDrift {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignWorkspace { expected, actual } => write!(
                f,
                "the gate ran in another workspace (`{expected}`, now `{actual}`)"
            ),
            Self::Scheme { before, after } => write!(
                f,
                "the source hashing scheme changed since the gate ran ({before}, now {after})"
            ),
            Self::SelectionPolicy { before, after } => write!(
                f,
                "the selection policy changed since the gate ran (receipt {}…, now {}…)",
                short(before),
                short(after)
            ),
            Self::Source { before, after } => write!(
                f,
                "the source changed since the gate ran (receipt {}…, now {}…)",
                short(before),
                short(after)
            ),
            Self::Configuration { before, after } => write!(
                f,
                "the applicable external configuration changed since the gate ran (receipt {}…, now {}…)",
                short(before),
                short(after)
            ),
            Self::Unbound { identity } => write!(
                f,
                "the latest attempt was recorded before {identity} evidence was bound to receipts; run the gate again"
            ),
        }
    }
}
