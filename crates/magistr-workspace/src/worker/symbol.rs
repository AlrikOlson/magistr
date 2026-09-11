use serde::{Deserialize, Serialize};

/// Extraction symbol with an owned kind for process transport.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerSymbol {
    pub name: String,
    pub kind: String,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub detail: Option<String>,
    pub doc: Option<String>,
}

impl From<crate::indexer::Symbol> for WorkerSymbol {
    fn from(symbol: crate::indexer::Symbol) -> Self {
        Self {
            name: symbol.name,
            kind: symbol.kind.to_owned(),
            start_line: symbol.start_line,
            start_col: symbol.start_col,
            end_line: symbol.end_line,
            detail: symbol.detail,
            doc: symbol.doc,
        }
    }
}
