use serde::{Deserialize, Serialize};

/// Inclusive source line range classified by extraction as test code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerTestRange {
    pub start_line: u32,
    pub end_line: u32,
}

impl From<crate::indexer::TestRange> for WorkerTestRange {
    fn from(range: crate::indexer::TestRange) -> Self {
        Self {
            start_line: range.start_line,
            end_line: range.end_line,
        }
    }
}
