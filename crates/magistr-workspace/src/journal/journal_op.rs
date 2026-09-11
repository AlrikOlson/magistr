use super::JournalFileRecord;

/// One journal-assigned mutation in append order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JournalOp {
    pub seq: i64,
    pub at: i64,
    pub tool: String,
    pub args_hash: String,
    pub session_id: String,
    /// Present only for undo operations; values are sorted ascending on read.
    pub undo_of: Option<Vec<i64>>,
    pub files: Vec<JournalFileRecord>,
}
