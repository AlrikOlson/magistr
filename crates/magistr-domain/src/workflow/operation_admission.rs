use crate::identity::AttemptId;

use super::OperationRecord;

/// What admitting a command into the ledger produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationAdmission {
    /// A new attempt to carry out; write the ledger before the effect.
    Accepted(AttemptId),
    /// The command already has an established `Completed` outcome; nothing
    /// is redone and this is the record that establishes it.
    Established(OperationRecord),
}

impl OperationAdmission {
    /// The attempt to run, or `None` when the outcome is already established.
    #[must_use]
    pub fn accepted(&self) -> Option<&AttemptId> {
        match self {
            Self::Accepted(attempt) => Some(attempt),
            Self::Established(_) => None,
        }
    }
}
