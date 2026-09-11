use serde::{Deserialize, Serialize};

use super::SearchHit;

/// Ranked file group; total hits survives display deduplication and capping.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchGroup {
    pub file: String,
    pub junk: bool,
    pub total_hits: usize,
    pub hits: Vec<SearchHit>,
}
