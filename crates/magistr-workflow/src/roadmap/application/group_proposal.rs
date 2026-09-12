use crate::roadmap::region::Cluster;

/// Suggested clusters with no changes to roadmap placement.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GroupProposal {
    pub clusters: Vec<Cluster>,
    pub min_shared: usize,
    pub groups: Vec<String>,
    pub placed: usize,
}
