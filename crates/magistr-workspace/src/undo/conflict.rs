/// Three-way hashes for a file whose observed bytes differ from history.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UndoConflict {
    pub path: String,
    pub op_seq: i64,
    /// None means absent; an empty file still has a SHA-256 address.
    pub expected_hash: Option<String>,
    pub current_hash: Option<String>,
    pub restore_hash: Option<String>,
}
