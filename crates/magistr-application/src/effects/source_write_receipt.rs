use alloc::string::String;

/// What the workspace pipeline truthfully did with a routed write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceWriteReceipt {
    /// The journal intent the write was recorded under; `None` when the
    /// bytes already matched and nothing was journaled.
    pub journal_seq: Option<i64>,
    /// The mutation receipt's revision over the applied file, read from the
    /// resolved intent; `None` when nothing changed.
    pub revision: Option<String>,
    /// Whether any byte changed on disk.
    pub changed: bool,
}
