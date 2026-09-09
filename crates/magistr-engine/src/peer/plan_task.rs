use serde::{Deserialize, Serialize};

use super::Peer;

/// A planned unit with one supervising peer and dependencies within this revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanTask {
    pub id: String,
    pub description: String,
    pub dependencies: Vec<String>,
    pub owner: Peer,
    pub acceptance: Vec<String>,
    pub risks: Vec<String>,
}
