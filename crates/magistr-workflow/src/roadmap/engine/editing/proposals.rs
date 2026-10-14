use super::*;

impl Edit<'_> {
    pub fn propose_status(
        &mut self,
        id: &str,
        suggested_status: ChunkStatus,
        reason: String,
        source: String,
    ) -> Result<&Chunk, String> {
        let idx = self.index_of(id)?;
        let unchanged = self.roadmap.chunks[idx]
            .status_proposal
            .as_ref()
            .is_some_and(|p| {
                p.suggested_status == suggested_status && p.reason == reason && p.source == source
            });
        if unchanged {
            return Ok(&self.roadmap.chunks[idx]);
        }

        let c = &mut self.roadmap.chunks[idx];
        c.status_proposal = Some(crate::roadmap::domain::StatusProposal {
            suggested_status,
            reason,
            proposed_at: Self::now(),
            source,
        });
        c.updated_at = Self::now();
        let snapshot = self.roadmap.chunks[idx].clone();
        self.record_event(RoadmapFrame::ChunkChanged { chunk: snapshot });
        Ok(&self.roadmap.chunks[idx])
    }

    pub fn propose_title(
        &mut self,
        id: &str,
        suggested_title: String,
        reason: String,
        source: String,
    ) -> Result<&Chunk, String> {
        let idx = self.index_of(id)?;
        let unchanged = self.roadmap.chunks[idx]
            .title_proposal
            .as_ref()
            .is_some_and(|p| {
                p.suggested_title == suggested_title && p.reason == reason && p.source == source
            });
        if unchanged {
            return Ok(&self.roadmap.chunks[idx]);
        }

        let c = &mut self.roadmap.chunks[idx];
        c.title_proposal = Some(crate::roadmap::domain::TitleProposal {
            suggested_title,
            reason,
            proposed_at: Self::now(),
            source,
        });
        c.updated_at = Self::now();
        let snapshot = self.roadmap.chunks[idx].clone();
        self.record_event(RoadmapFrame::ChunkChanged { chunk: snapshot });
        Ok(&self.roadmap.chunks[idx])
    }

    pub fn resolve_title_proposal(&mut self, id: &str, accept: bool) -> Result<&Chunk, String> {
        let idx = self.index_of(id)?;
        let Some(proposal) = self.roadmap.chunks[idx].title_proposal.take() else {
            return Err(format!("chunk '{id}' has no open title proposal"));
        };
        let c = &mut self.roadmap.chunks[idx];
        if accept {
            c.title = proposal.suggested_title;
        }
        c.updated_at = Self::now();
        let snapshot = self.roadmap.chunks[idx].clone();
        self.record_event(RoadmapFrame::ChunkChanged { chunk: snapshot });
        Ok(&self.roadmap.chunks[idx])
    }

    pub fn propose_reprioritize(
        &mut self,
        id: &str,
        suggested_priority: u32,
        reason: String,
    ) -> Result<&Chunk, String> {
        let idx = self.index_of(id)?;
        let unchanged = self.roadmap.chunks[idx]
            .reprioritize
            .as_ref()
            .is_some_and(|p| p.suggested_priority == suggested_priority && p.reason == reason);
        if unchanged {
            return Ok(&self.roadmap.chunks[idx]);
        }

        let c = &mut self.roadmap.chunks[idx];
        c.reprioritize = Some(ReprioritizeProposal {
            suggested_priority,
            reason,
            proposed_at: Self::now(),
        });
        c.updated_at = Self::now();
        let snapshot = self.roadmap.chunks[idx].clone();
        self.record_event(RoadmapFrame::ChunkChanged { chunk: snapshot });
        Ok(&self.roadmap.chunks[idx])
    }

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
        let chunk = self.roadmap.chunks[index].clone();
        self.record_event(RoadmapFrame::ChunkChanged { chunk });
        Ok(&self.roadmap.chunks[index])
    }

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
