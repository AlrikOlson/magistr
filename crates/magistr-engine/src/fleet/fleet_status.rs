use serde::{Deserialize, Serialize};

/// What the control plane can say about the whole fleet at one instant.
///
/// The two questions an operator actually asks are "is anything down?"
/// and "is everything running what I published?", so the rollup answers
/// both and keeps them separate: health comes from leases, convergence
/// comes from digests, and a fleet can fail either one on its own.
///
/// `drifted` names the nodes rather than counting them, because the
/// remedy is per-node and a count is not actionable.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetStatus {
    /// Nodes known to the control plane.
    pub total: usize,
    /// Nodes that renewed inside the lease TTL.
    pub ready: usize,
    /// Nodes that missed a renewal but are inside the miss allowance.
    pub stale: usize,
    /// Nodes past the allowance.
    pub unreachable: usize,
    /// The generation currently published (0 = nothing published).
    pub generation: u64,
    /// The digest of the current bundle.
    pub digest: String,
    /// Nodes running exactly the current bundle.
    pub on_current: usize,
    /// Ids of the nodes that are not, in id order.
    pub drifted: Vec<String>,
}

impl FleetStatus {
    /// Nodes that still count as part of the fleet.
    #[must_use]
    pub fn live(&self) -> usize {
        self.ready + self.stale
    }

    /// Whether every node is reachable and running the current bundle.
    ///
    /// Deliberately strict about unreachable nodes: a node the control
    /// plane cannot see may be running anything, so a fleet with one
    /// gone machine has not converged, whatever that machine last said.
    #[must_use]
    pub fn is_converged(&self) -> bool {
        self.drifted.is_empty() && self.unreachable == 0
    }
}

#[cfg(test)]
#[path = "fleet_status.tests.rs"]
mod tests;
