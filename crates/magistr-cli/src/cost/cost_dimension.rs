//! The axis a cost report groups spend along.

use super::cost_entry::CostEntry;

/// The axis a cost report groups spend along.
///
/// `Project` and `Session` always have a key; `Actor` and `Model` may
/// not, because older entries may carry no attribution. Those
/// group under [`CostDimension::UNATTRIBUTED`] rather than being dropped
/// — a chargeback report that silently loses money is worse than one
/// that admits it does not know whose it was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CostDimension {
    /// One row per project.
    Project,
    /// One row per team identity.
    Actor,
    /// One row per model.
    Model,
    /// One row per session.
    Session,
}

impl CostDimension {
    /// The key spend with no attribution is charged to.
    pub const UNATTRIBUTED: &'static str = "unattributed";

    /// The name of this dimension.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            CostDimension::Project => "project",
            CostDimension::Actor => "actor",
            CostDimension::Model => "model",
            CostDimension::Session => "session",
        }
    }

    /// The group key an entry falls under.
    #[must_use]
    pub fn key_of(&self, entry: &CostEntry) -> String {
        match self {
            CostDimension::Project => entry.project.clone(),
            CostDimension::Session => entry.session_id.clone(),
            CostDimension::Actor => entry
                .actor
                .clone()
                .unwrap_or_else(|| Self::UNATTRIBUTED.to_string()),
            CostDimension::Model => entry
                .model
                .clone()
                .unwrap_or_else(|| Self::UNATTRIBUTED.to_string()),
        }
    }
}

impl std::fmt::Display for CostDimension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
#[path = "cost_dimension.tests.rs"]
mod tests;
