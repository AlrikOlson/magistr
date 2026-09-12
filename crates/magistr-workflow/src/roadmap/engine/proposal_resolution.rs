use super::{Chunk, RoadmapEngine, RoadmapFrame};

impl RoadmapEngine {
    /// Resolve a priority proposal in one persisted mutation, including a
    /// rejection or an accepted suggestion equal to the existing priority.
    pub fn resolve_reprioritize(&mut self, id: &str, accept: bool) -> Result<&Chunk, String> {
        let index = self.index_of(id)?;
        let Some(proposal) = self.roadmap.chunks[index].reprioritize.take() else {
            return Err(format!("chunk '{id}' has no open reprioritize proposal"));
        };
        let chunk = &mut self.roadmap.chunks[index];
        if accept {
            chunk.priority = proposal.suggested_priority;
        }
        chunk.updated_at = Self::now();
        self.persist();
        let chunk = self.roadmap.chunks[index].clone();
        self.record_event(RoadmapFrame::ChunkChanged { chunk });
        Ok(&self.roadmap.chunks[index])
    }

    /// Read the proposal and settle its status under the same native lock.
    pub fn resolve_status_proposal(&mut self, id: &str, accept: bool) -> Result<&Chunk, String> {
        let index = self.index_of(id)?;
        let chunk = &self.roadmap.chunks[index];
        let Some(proposal) = &chunk.status_proposal else {
            return Err(format!("chunk '{id}' has no open status proposal"));
        };
        let status = if accept {
            proposal.suggested_status
        } else {
            chunk.status
        };
        self.set_status(id, status)
    }
}

#[cfg(test)]
#[path = "proposal_resolution.tests.rs"]
mod tests;
