use super::{
    MapCargoSummary, MapCrateRole, MapDependency, MapEntryPoint, MapImportGraph, MapIndex,
    MapPlugin, MapSymbol,
};
use serde::Serialize;
use std::collections::BTreeMap;

/// Ordered overview, ranked Cargo members and breadth-first tree rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "lowercase",
    rename_all_fields = "camelCase"
)]
pub enum MapRow {
    Overview {
        path: String,
        index: Option<MapIndex>,
        ttl_ms: u64,
        files: usize,
        dirs: usize,
        languages: BTreeMap<String, usize>,
        plugins: Vec<MapPlugin>,
        #[serde(skip_serializing_if = "Option::is_none")]
        import_graph: Option<MapImportGraph>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        entry_points: Vec<MapEntryPoint>,
        #[serde(skip_serializing_if = "Option::is_none")]
        cargo: Option<MapCargoSummary>,
    },
    Crate {
        path: String,
        name: String,
        roles: Vec<MapCrateRole>,
        #[serde(skip_serializing_if = "Option::is_none")]
        edition: Option<String>,
        entry_points: Vec<MapEntryPoint>,
        #[serde(skip_serializing_if = "Option::is_none")]
        more_entry_points: Option<usize>,
        features: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        more_features: Option<usize>,
        dependencies: Vec<MapDependency>,
        dependents: usize,
        external_dependencies: usize,
    },
    Dir {
        path: String,
        files: usize,
        dirs: usize,
    },
    File {
        path: String,
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        junk: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        language: Option<String>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        symbols: Vec<MapSymbol>,
        #[serde(skip_serializing_if = "Option::is_none")]
        dropped_symbols: Option<usize>,
    },
}

impl MapRow {
    pub fn path(&self) -> &str {
        match self {
            Self::Overview { path, .. }
            | Self::Crate { path, .. }
            | Self::Dir { path, .. }
            | Self::File { path, .. } => path,
        }
    }
}
