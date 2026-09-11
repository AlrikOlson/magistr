use super::JournalOp;

/// Log eligibility excludes undo and consumed operations; missing blobs are
/// represented separately on each file, as in the retained journal port.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JournalLogEntry {
    pub op: JournalOp,
    pub undoable: bool,
    pub undone_by: Option<i64>,
    /// Native outcomes are absent for retained legacy operations.
    pub write_outcome: Option<super::JournalWriteOutcome>,
}
