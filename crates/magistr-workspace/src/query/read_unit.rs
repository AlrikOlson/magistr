use super::{ReadRange, ReadSymbolRef};
use serde::Serialize;

/// The resolved unit and SHA-256 of the full, original file bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadUnit {
    pub file: String,
    pub range: ReadRange,
    pub total_lines: usize,
    pub content_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<ReadSymbolRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enclosing: Option<ReadSymbolRef>,
}
