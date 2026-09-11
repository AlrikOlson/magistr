use serde::{Deserialize, Serialize};

/// Regex hit with offsets relative to its unchanged input line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerSearchHit {
    pub line: u64,
    pub start: u32,
    pub end: u32,
    pub line_text: String,
}

impl From<crate::engine::SearchHit> for WorkerSearchHit {
    fn from(hit: crate::engine::SearchHit) -> Self {
        Self {
            line: hit.line,
            start: hit.start,
            end: hit.end,
            line_text: hit.line_text,
        }
    }
}
