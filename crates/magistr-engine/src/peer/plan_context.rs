use serde::{Deserialize, Serialize};

/// References to the same immutable evidence delivered to both peers.
/// The host resolves these references and verifies both peers received that evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanContext {
    pub roadmap_revision: String,
    pub code_revision: String,
    pub constraints: Vec<String>,
    pub routing_history_revision: String,
}
