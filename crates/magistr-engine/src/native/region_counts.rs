use serde::{Deserialize, Serialize};

/// Per-workstream chunk counts by status (obsoleted chunks excluded).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegionCounts {
    pub total: u32,
    pub backlog: u32,
    pub pending: u32,
    pub in_progress: u32,
    pub blocked: u32,
    pub done: u32,
}

impl RegionCounts {
    /// Whether every live chunk in the region is done.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.total > 0 && self.done == self.total
    }
}
