use crate::roadmap::domain::{Chunk, ChunkStatus};
use magistr_workflow_contracts::Observed;

use super::{RoadmapError, RoadmapUseCases};

impl RoadmapUseCases {
    pub fn propose_title(
        &self,
        id: &str,
        title: String,
        reason: String,
        source: String,
    ) -> Result<Observed<Chunk>, RoadmapError> {
        self.lock()?
            .propose_title(id, title, reason, source)
            .map_err(RoadmapError::invalid_state)
    }

    pub fn propose_status(
        &self,
        id: &str,
        status: ChunkStatus,
        reason: String,
        source: String,
    ) -> Result<Observed<Chunk>, RoadmapError> {
        self.lock()?
            .propose_status(id, status, reason, source)
            .map_err(RoadmapError::invalid_state)
    }

    pub fn propose_reprioritize(
        &self,
        id: &str,
        priority: u32,
        reason: String,
    ) -> Result<Observed<Chunk>, RoadmapError> {
        self.lock()?
            .propose_reprioritize(id, priority, reason)
            .map_err(RoadmapError::invalid_state)
    }

    pub fn resolve_title_proposal(
        &self,
        id: &str,
        accept: bool,
    ) -> Result<Observed<Chunk>, RoadmapError> {
        self.lock()?
            .resolve_title_proposal(id, accept)
            .map_err(RoadmapError::invalid_state)
    }

    pub fn resolve_status_proposal(
        &self,
        id: &str,
        accept: bool,
    ) -> Result<Observed<Chunk>, RoadmapError> {
        self.lock()?
            .mutate(|edit| {
                if accept
                    && edit
                        .roadmap()
                        .chunks
                        .iter()
                        .find(|chunk| chunk.id == id)
                        .and_then(|chunk| chunk.status_proposal.as_ref())
                        .is_some_and(|proposal| proposal.suggested_status == ChunkStatus::Done)
                {
                    return Err(Self::refuse_direct_done(id).to_string());
                }
                edit.resolve_status_proposal(id, accept).cloned()
            })
            .map_err(RoadmapError::invalid_state)
    }

    pub fn resolve_reprioritize(
        &self,
        id: &str,
        accept: bool,
    ) -> Result<Observed<Chunk>, RoadmapError> {
        self.lock()?
            .resolve_reprioritize(id, accept)
            .map_err(RoadmapError::invalid_state)
    }
}
