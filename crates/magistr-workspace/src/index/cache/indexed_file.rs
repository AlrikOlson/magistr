use crate::worker::{WorkerSymbol, WorkerTestRange};

/// One indexed file, or its metadata when returned by a bounded list query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedFile {
    pub path: String,
    pub language: Option<String>,
    pub size: u64,
    pub mtime_ms: i64,
    pub mode: u32,
    pub content_hash: String,
    pub junk: bool,
    pub content: Option<String>,
    pub symbols: Vec<WorkerSymbol>,
    pub imports: Vec<String>,
    pub test_ranges: Vec<WorkerTestRange>,
}
