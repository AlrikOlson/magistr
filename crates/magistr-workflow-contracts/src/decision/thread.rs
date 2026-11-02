//! Thread types — forks off the main reasoning line and their lifecycle.

use serde::{Deserialize, Serialize};

use super::note::Note;

/// Lifecycle state of a thread. A thread always starts `Open`; the agent
/// can mark it `Merged` when its conclusions have been folded into the
/// main line, or `Abandoned` when the approach is dropped.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ThreadStatus {
    Open,
    Merged,
    Abandoned,
}

/// A named fork off the main reasoning line. The thread's `from_note`
/// points at the main-line note it forks from; subsequent notes with
/// `thread_id == this.id` belong to the thread.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
pub struct Thread {
    pub id: String,
    pub name: String,
    pub from_note: u32,
    pub notes: Vec<Note>,
    pub status: ThreadStatus,
    pub created_at: String,
    pub depth: u32,
    /// Set when the thread is marked `merged` and a synthesis note is
    /// named — the note that aggregates this thread's conclusions back
    /// into the main reasoning line. Lets `impact_of` answer "where did
    /// this thread end up?" in one call.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub merged_into: Option<u32>,
}

/// The exact lifecycle metadata acknowledged by the reasoning owner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ThreadStatusChange {
    pub previous: String,
    pub current: String,
    pub merged_into: Option<u32>,
}

#[cfg(test)]
#[path = "thread.tests.rs"]
mod tests;
