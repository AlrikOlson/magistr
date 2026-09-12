use serde::{Deserialize, Serialize};

use super::ChangeKind;

/// One path whose observation changed between a snapshot and a validation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourceChange {
    pub path: String,
    pub kind: ChangeKind,
}
