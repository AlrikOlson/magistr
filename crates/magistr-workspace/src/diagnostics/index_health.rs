use serde::Serialize;

use crate::index::IndexStatus;

/// The derived index as a status row. Generation is cache progress, never a
/// source revision or completion evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexHealth {
    pub freshness: &'static str,
    pub building: bool,
    pub generation: u64,
    pub files: usize,
    pub parsed: usize,
    pub reused: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    pub notes: Vec<String>,
}

impl From<IndexStatus> for IndexHealth {
    fn from(status: IndexStatus) -> Self {
        Self {
            freshness: status.freshness.as_str(),
            building: status.building,
            generation: status.generation,
            files: status.files,
            parsed: status.parsed,
            reused: status.reused,
            last_error: status.last_error,
            notes: status.notes,
        }
    }
}
