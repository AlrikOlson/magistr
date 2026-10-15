//! Branch types — forks off the main reasoning line and their lifecycle.

use serde::{Deserialize, Serialize};

use super::step::ThinkStep;

/// Lifecycle state of a branch. A branch always starts `Active`; the agent
/// can mark it `Merged` when its conclusions have been folded into the
/// main line, or `Abandoned` when the approach is dropped.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum BranchStatus {
    Active,
    Merged,
    Abandoned,
}

/// A named fork off the main reasoning line. The branch's `from_step`
/// points at the main-line step it forks from; subsequent steps with
/// `branch_id == this.id` belong to the branch.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct Branch {
    pub id: String,
    pub name: String,
    pub from_step: u32,
    pub steps: Vec<ThinkStep>,
    pub status: BranchStatus,
    pub created_at: String,
    pub depth: u32,
    /// Set when the branch is marked `merged` and a synthesis step is
    /// named — the step that aggregates this branch's conclusions back
    /// into the main reasoning line. Lets `impact_of` answer "where did
    /// this branch end up?" in one call.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub merged_into: Option<u32>,
}

/// The exact lifecycle metadata acknowledged by the reasoning owner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BranchStatusChange {
    pub previous: String,
    pub current: String,
    pub merged_into: Option<u32>,
}

#[cfg(test)]
#[path = "branch.tests.rs"]
mod tests;
