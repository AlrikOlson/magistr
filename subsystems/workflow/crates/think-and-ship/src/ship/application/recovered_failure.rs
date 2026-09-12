use serde::{Deserialize, Serialize};

/// A historical failure of a gate that a later attempt in the same run
/// superseded with a pass: reported so the summary is honest about the
/// path, and never counted against completion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveredFailure {
    pub gate: String,
    pub receipt_id: String,
    pub attempt: u64,
    pub outcome: String,
}
