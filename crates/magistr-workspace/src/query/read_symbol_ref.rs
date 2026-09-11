use crate::worker::WorkerSymbol;
use serde::Serialize;

/// Outline identity attached to a read header.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadSymbolRef {
    pub name: String,
    pub kind: String,
    pub line: u32,
    pub end_line: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
}

impl From<&WorkerSymbol> for ReadSymbolRef {
    fn from(symbol: &WorkerSymbol) -> Self {
        Self {
            name: symbol.name.clone(),
            kind: symbol.kind.clone(),
            line: symbol.start_line,
            end_line: symbol.end_line,
            doc: symbol.doc.clone(),
        }
    }
}
