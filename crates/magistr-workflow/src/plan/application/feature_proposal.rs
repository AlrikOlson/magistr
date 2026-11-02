use crate::plan::region::Cluster;

/// Suggested clusters with no changes to plan placement.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FeatureProposal {
    pub clusters: Vec<Cluster>,
    pub min_shared: usize,
    /// All authored names, including history, for avoiding naming collisions.
    pub authored_names: Vec<String>,
    pub placed: usize,
}
