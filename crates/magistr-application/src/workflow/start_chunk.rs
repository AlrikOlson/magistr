use crate::CommandContext;

use super::ChunkReference;

/// Start one ready chunk through the native lifecycle authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartChunk {
    pub command: CommandContext,
    pub chunk: ChunkReference,
}
