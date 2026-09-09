use serde::{Deserialize, Serialize};

/// Agreed behavior for an unavailable peer. Neither variant treats silence as a vote.
/// The host must checkpoint affected work and confirm stops before reassignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutagePolicy {
    CheckpointAndStop,
    CheckpointAffectedAndContinueIndependent,
}
