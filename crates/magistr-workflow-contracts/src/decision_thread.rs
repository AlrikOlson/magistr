//! Persisted thread metadata without duplicating its reasoning notes.

use serde::{Deserialize, Serialize};

use crate::decision::{Thread, ThreadStatus};

/// A question's recorded identity, origin and lifecycle in a native snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionThread {
    pub id: String,
    pub name: String,
    pub from_note: Option<u32>,
    pub status: ThreadStatus,
    /// The recorded synthesis note; absent means the destination was not recorded.
    pub merged_into: Option<u32>,
}

impl From<&Thread> for DecisionThread {
    fn from(thread: &Thread) -> Self {
        Self {
            id: thread.id.clone(),
            name: thread.name.clone(),
            from_note: Some(thread.from_note),
            status: thread.status,
            merged_into: thread.merged_into,
        }
    }
}
