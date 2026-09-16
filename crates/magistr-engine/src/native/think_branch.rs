//! Persisted branch metadata without duplicating its reasoning steps.

use magistr_workflow::think::domain::{Branch, BranchStatus};

/// A question's recorded identity, origin and lifecycle in a native snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThinkBranch {
    pub id: String,
    pub name: String,
    pub from_step: Option<u32>,
    pub status: BranchStatus,
    /// The recorded synthesis step; absent means the destination was not recorded.
    pub merged_into: Option<u32>,
}

impl From<&Branch> for ThinkBranch {
    fn from(branch: &Branch) -> Self {
        Self {
            id: branch.id.clone(),
            name: branch.name.clone(),
            from_step: Some(branch.from_step),
            status: branch.status,
            merged_into: branch.merged_into,
        }
    }
}
