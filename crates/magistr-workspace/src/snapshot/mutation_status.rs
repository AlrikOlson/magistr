use serde::{Deserialize, Serialize};

/// What a journaled mutation truthfully did to the filesystem.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationStatus {
    /// Every swap succeeded and read back; the durable intent is resolved applied.
    Applied,
    /// Nothing of it remains: the intent was rolled back after a failed swap.
    Failed,
    /// The truth is not known from the journal alone: the owner was
    /// interrupted before resolving the intent, compensation or outcome
    /// persistence failed, or the operation predates native intents.
    Indeterminate,
}
