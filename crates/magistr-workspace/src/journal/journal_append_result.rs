use super::JournalOp;

/// Committed operation and visible retention degradations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JournalAppendResult {
    pub op: JournalOp,
    pub notes: Vec<String>,
}
