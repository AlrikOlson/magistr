//! Accumulates routing decisions for transparency and debugging.

use super::routing_decision::RoutingDecision;

/// Accumulates routing decisions for transparency and debugging.
#[derive(Debug, Clone, Default)]
pub struct RoutingLog {
    /// All routing decisions made during this session.
    decisions: Vec<RoutingDecision>,
}

impl RoutingLog {
    /// Create an empty routing log.
    pub fn new() -> Self {
        Self {
            decisions: Vec::new(),
        }
    }

    /// Record a routing decision.
    pub fn record(&mut self, decision: RoutingDecision) {
        self.decisions.push(decision);
    }

    /// All recorded decisions.
    pub fn decisions(&self) -> &[RoutingDecision] {
        &self.decisions
    }

    /// Number of routing decisions recorded.
    pub fn len(&self) -> usize {
        self.decisions.len()
    }

    /// Whether the log is empty.
    pub fn is_empty(&self) -> bool {
        self.decisions.is_empty()
    }

    /// The most recent routing decision.
    pub fn last(&self) -> Option<&RoutingDecision> {
        self.decisions.last()
    }
}
