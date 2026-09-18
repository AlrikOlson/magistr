use serde::{Deserialize, Serialize};

/// Durable launch evidence, independent of a dispatch's eventual outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DispatchPhase {
    /// The transport has not crossed its durable pre-spawn boundary.
    Pending,
    /// The transport may have spawned; a missing handle proves nothing.
    Started,
}
