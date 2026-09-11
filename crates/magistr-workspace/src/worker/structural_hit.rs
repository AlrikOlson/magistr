use serde::{Deserialize, Serialize};

/// Structural hit in one native parser input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerStructuralHit {
    pub line: u64,
    pub column: u32,
    pub end_line: u64,
    pub kind: String,
    pub line_text: String,
}

impl From<crate::engine::StructuralHit> for WorkerStructuralHit {
    fn from(hit: crate::engine::StructuralHit) -> Self {
        Self {
            line: hit.line,
            column: hit.column,
            end_line: hit.end_line,
            kind: hit.kind,
            line_text: hit.line_text,
        }
    }
}
