use crate::roadmap::domain::{Chunk, ChunkStatus};

use super::{RoadmapError, RoadmapUseCases};

impl RoadmapUseCases {
    pub fn propose_title(
        &self,
        id: &str,
        title: String,
        reason: String,
        source: String,
    ) -> Result<Chunk, RoadmapError> {
        self.lock()?
            .propose_title(id, title, reason, source)
            .cloned()
            .map_err(RoadmapError::InvalidState)
    }

    pub fn propose_status(
        &self,
        id: &str,
        status: ChunkStatus,
        reason: String,
        source: String,
    ) -> Result<Chunk, RoadmapError> {
        self.lock()?
            .propose_status(id, status, reason, source)
            .cloned()
            .map_err(RoadmapError::InvalidState)
    }

    pub fn propose_reprioritize(
        &self,
        id: &str,
        priority: u32,
        reason: String,
    ) -> Result<Chunk, RoadmapError> {
        self.lock()?
            .propose_reprioritize(id, priority, reason)
            .cloned()
            .map_err(RoadmapError::InvalidState)
    }

    pub fn resolve_title_proposal(&self, id: &str, accept: bool) -> Result<Chunk, RoadmapError> {
        self.lock()?
            .resolve_title_proposal(id, accept)
            .cloned()
            .map_err(RoadmapError::InvalidState)
    }

    pub fn resolve_status_proposal(&self, id: &str, accept: bool) -> Result<Chunk, RoadmapError> {
        let mut engine = self.lock()?;
        if accept
            && engine
                .roadmap()
                .chunks
                .iter()
                .find(|chunk| chunk.id == id)
                .and_then(|chunk| chunk.status_proposal.as_ref())
                .is_some_and(|proposal| proposal.suggested_status == ChunkStatus::Done)
        {
            return Err(Self::refuse_direct_done(id));
        }
        engine
            .resolve_status_proposal(id, accept)
            .cloned()
            .map_err(RoadmapError::InvalidState)
    }

    pub fn resolve_reprioritize(&self, id: &str, accept: bool) -> Result<Chunk, RoadmapError> {
        self.lock()?
            .resolve_reprioritize(id, accept)
            .cloned()
            .map_err(RoadmapError::InvalidState)
    }
}
