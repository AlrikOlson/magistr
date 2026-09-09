use crate::CommandContext;

use super::{ChunkReference, TaskReference};

/// Complete a chunk with a real verification task resolved by the native owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteChunk {
    pub command: CommandContext,
    pub chunk: ChunkReference,
    pub verification_task: TaskReference,
}
