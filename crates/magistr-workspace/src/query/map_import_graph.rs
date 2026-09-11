use super::MapImportModule;
use serde::Serialize;

/// The top five source import targets and importing files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapImportGraph {
    pub fan_in: Vec<MapImportModule>,
    pub fan_out: Vec<MapImportModule>,
}
