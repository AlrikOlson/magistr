use serde::{Deserialize, Serialize};

/// The work a run is allowed to admit, independent of its presentation surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunMode {
    /// An ongoing conversation has no Git progress or iteration requirement.
    Conversation,
    /// Settle one selected chunk attempt and stop.
    OneChunk,
    /// Admit attempts within a persisted iteration and progress budget.
    ChunkLoop,
}
