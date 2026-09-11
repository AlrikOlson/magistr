use super::JournalFileChange;

/// A write batch before the journal assigns its sequence and timestamp.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JournalOpInit {
    pub tool: String,
    pub args_hash: String,
    pub session_id: String,
    pub files: Vec<JournalFileChange>,
    pub undo_of: Option<Vec<i64>>,
}
