/// A file skipped because its operation was evicted or its bytes were kept.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UndoSkippedFile {
    pub path: String,
    pub op_seq: i64,
}
