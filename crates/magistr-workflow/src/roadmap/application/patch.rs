use super::{ChunkPatch, RoadmapError, RoadmapUseCases};
use crate::roadmap::domain::Chunk;
use magistr_workflow_contracts::Observed;

impl RoadmapUseCases {
    /// Validate and apply the complete patch to one candidate under the store lock.
    pub fn update_chunk(
        &self,
        id: &str,
        patch: ChunkPatch,
    ) -> Result<Observed<Chunk>, RoadmapError> {
        if patch.blocked_by.is_some() && patch.clear_blocked_by {
            return Err(RoadmapError::InvalidArgs(
                "blocked_by and clear_blocked_by contradict each other — send one".into(),
            ));
        }
        if let Some(content) = &patch.content {
            content.validate().map_err(RoadmapError::InvalidArgs)?;
        }
        let mut engine = self.lock()?;
        let blocker = patch
            .blocked_by
            .map(|blocker| {
                engine.validate_blocked_by(blocker.kind, blocker.reason, blocker.evidence)
            })
            .transpose()
            .map_err(RoadmapError::InvalidArgs)?;
        engine
            .mutate(|edit| {
                if let Some(accept) = patch.resolve_title_proposal {
                    edit.resolve_title_proposal(id, accept)?;
                }
                if let Some(name) = patch.name.as_deref() {
                    edit.set_name(id, name)?;
                }
                if let Some(blocker) = blocker {
                    edit.set_blocked_by(id, blocker)?;
                }
                if patch.clear_blocked_by {
                    edit.clear_blocked_by(id)?;
                }
                edit.update_chunk(
                    id,
                    patch.title,
                    patch.priority,
                    patch.description,
                    patch.acceptance,
                    patch.deps,
                    patch.content,
                )
                .cloned()
            })
            .map_err(RoadmapError::invalid_state)
    }
}
