use serde::Serialize;

/// Source symbol whose line anchor was checked against the current file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MapSymbol {
    pub name: String,
    pub kind: String,
    pub line: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
}
