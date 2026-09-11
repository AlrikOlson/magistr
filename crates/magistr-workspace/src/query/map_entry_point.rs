use super::MapEntryKind;
use serde::Serialize;

/// Workspace-relative entry point, retaining any declared target name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MapEntryPoint {
    pub path: String,
    pub kind: MapEntryKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}
