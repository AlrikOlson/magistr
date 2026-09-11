use super::MapDetail;
use serde::Deserialize;

/// Bounded workspace orientation; depth is relative to the admitted path.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct MapInput {
    pub path: String,
    pub depth: usize,
    pub detail: MapDetail,
    pub budget: u64,
}

impl Default for MapInput {
    fn default() -> Self {
        Self {
            path: ".".into(),
            depth: 3,
            detail: MapDetail::Outline,
            budget: 6000,
        }
    }
}
