use crate::roadmap::region::Cluster;

/// Suggested clusters with no changes to roadmap placement.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GroupProposal {
    pub clusters: Vec<Cluster>,
    pub min_shared: usize,
    /// All authored names, including history, for avoiding naming collisions.
    pub authored_names: Vec<String>,
    pub placed: usize,
}
