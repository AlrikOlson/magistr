use serde::Serialize;

/// Index health associated with this answer, never source certification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapIndex {
    pub tier: u8,
    pub building: bool,
    pub generation: u64,
    pub files_total: usize,
    pub files_indexed: usize,
}
