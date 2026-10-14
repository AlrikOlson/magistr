use super::{Chunk, Observed, RoadmapEngine, RoadmapMutationError};

impl RoadmapEngine {
    pub fn resolve_reprioritize(
        &mut self,
        id: &str,
        accept: bool,
    ) -> Result<Observed<Chunk>, RoadmapMutationError> {
        self.mutate(|edit| edit.resolve_reprioritize(id, accept).cloned())
    }

    pub fn resolve_status_proposal(
        &mut self,
        id: &str,
        accept: bool,
    ) -> Result<Observed<Chunk>, RoadmapMutationError> {
        self.mutate(|edit| edit.resolve_status_proposal(id, accept).cloned())
    }
}

#[cfg(test)]
#[path = "proposal_resolution.tests.rs"]
mod tests;
