use super::MapDependent;
use serde::Serialize;

/// Whole-workspace Cargo counts, even when the tree is scoped to one member.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapCargoSummary {
    pub kind: String,
    pub members: usize,
    pub edges: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub omitted: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unused_deps: Option<usize>,
    pub top: Vec<MapDependent>,
}
