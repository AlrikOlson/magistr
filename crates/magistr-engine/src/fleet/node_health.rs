use serde::{Deserialize, Serialize};

/// How alive a fleet node is — *derived* from its lease, never asserted.
///
/// A node that says "I am healthy" and then dies says nothing further;
/// a node that renews a lease stops renewing it the moment it dies. So
/// health here is a function of one timestamp and the lease TTL, the
/// same shape Kubernetes gives a Node through its `Lease` object.
///
/// Between [`Ready`] and [`Unreachable`] sits [`Stale`]: a node that has
/// missed a renewal but not enough of them to declare it gone. The
/// allowance exists because a single missed heartbeat is far more often
/// a slow network than a dead machine, and evicting on the first miss
/// makes a fleet flap.
///
/// [`Ready`]: NodeHealth::Ready
/// [`Stale`]: NodeHealth::Stale
/// [`Unreachable`]: NodeHealth::Unreachable
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeHealth {
    /// Renewed within the lease TTL.
    Ready,
    /// Missed at least one renewal, still inside the miss allowance.
    Stale,
    /// Past the allowance — treat the node as gone.
    Unreachable,
}

impl NodeHealth {
    /// How many lease periods a node may miss before it is unreachable.
    pub const MISS_ALLOWANCE: i64 = 3;

    /// Derive health from a lease age (seconds since the last renewal)
    /// and the fleet's lease TTL.
    ///
    /// A negative age — a node whose clock runs ahead of the control
    /// plane's — reads as a fresh renewal rather than an error: clock
    /// skew must not make a live node look dead.
    #[must_use]
    pub fn from_lease(age_secs: i64, lease_ttl_secs: i64) -> Self {
        let ttl = lease_ttl_secs.max(1);
        if age_secs <= ttl {
            Self::Ready
        } else if age_secs <= ttl.saturating_mul(Self::MISS_ALLOWANCE) {
            Self::Stale
        } else {
            Self::Unreachable
        }
    }

    /// Whether the node still counts as part of the fleet.
    #[must_use]
    pub fn is_live(self) -> bool {
        !matches!(self, Self::Unreachable)
    }

    /// The wire spelling, for status rows and protocol results.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Stale => "stale",
            Self::Unreachable => "unreachable",
        }
    }
}

#[cfg(test)]
#[path = "node_health.tests.rs"]
mod tests;
