//! Typed mutation records; emission belongs to the workflow host.

use super::Chunk;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RoadmapFrame {
    ChunkAdded {
        chunk: Chunk,
    },
    /// A non-terminal change (status/update/reprioritize/link).
    ChunkChanged {
        chunk: Chunk,
    },
    /// Terminal: the chunk shipped. Triggers a session commit when shared.
    ChunkCompleted {
        chunk: Chunk,
    },
    /// Terminal: the chunk was retired. Triggers a session commit when shared.
    ChunkObsoleted {
        chunk: Chunk,
    },
    RefreshRecorded {
        summary: String,
        think_steps: Vec<u32>,
    },
}

impl RoadmapFrame {
    /// `(kind, closes_session)` for the git-native record mapping.
    /// `kind` is the Agent Trace record discriminator; `closes_session` marks
    /// the chunk-lifecycle close events that trigger a commit.
    pub fn record_meta(&self) -> (&'static str, bool) {
        match self {
            Self::ChunkAdded { .. } | Self::ChunkChanged { .. } => ("chunk", false),
            Self::ChunkCompleted { .. } | Self::ChunkObsoleted { .. } => ("chunk", true),
            Self::RefreshRecorded { .. } => ("refresh", false),
        }
    }
}
