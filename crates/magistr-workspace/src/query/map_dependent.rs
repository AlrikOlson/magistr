use serde::Serialize;

/// One crate in the workspace-wide dependent ranking.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MapDependent {
    pub name: String,
    pub dependents: usize,
}
