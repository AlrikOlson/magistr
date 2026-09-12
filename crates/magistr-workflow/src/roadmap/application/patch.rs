use crate::roadmap::domain::Chunk;

use super::{ChunkPatch, RoadmapError, RoadmapUseCases};

impl RoadmapUseCases {
    /// Apply the established proposal, label, blocker, descriptive-patch order
    /// under one native lock. Content validation precedes every mutation.
    pub fn update_chunk(&self, id: &str, patch: ChunkPatch) -> Result<Chunk, RoadmapError> {
        if let Some(content) = &patch.content {
            content.validate().map_err(RoadmapError::InvalidArgs)?;
        }
        let mut engine = self.lock()?;
        if let Some(accept) = patch.resolve_title_proposal {
            engine
                .resolve_title_proposal(id, accept)
                .map_err(RoadmapError::InvalidState)?;
        }
        if let Some(name) = patch.name.as_deref() {
            engine
                .set_name(id, name)
                .map_err(RoadmapError::InvalidArgs)?;
        }
        if patch.blocked_by.is_some() && patch.clear_blocked_by {
            return Err(RoadmapError::InvalidArgs(
                "blocked_by and clear_blocked_by contradict each other — send one".into(),
            ));
        }
        if let Some(blocker) = patch.blocked_by {
            let blocker = engine
                .validate_blocked_by(blocker.kind, blocker.reason, blocker.evidence)
                .map_err(RoadmapError::InvalidArgs)?;
            engine
                .set_blocked_by(id, blocker)
                .map_err(RoadmapError::InvalidState)?;
        }
        if patch.clear_blocked_by {
            engine
                .clear_blocked_by(id)
                .map_err(RoadmapError::InvalidState)?;
        }
        engine
            .update_chunk(
                id,
                patch.title,
                patch.priority,
                patch.description,
                patch.acceptance,
                patch.deps,
                patch.content,
            )
            .cloned()
            .map_err(RoadmapError::InvalidState)
    }
}
