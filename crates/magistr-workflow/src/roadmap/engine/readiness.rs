//! Scheduling policy is shared; the writer supplies current records and names.
use super::{Chunk, RoadmapEngine};
use magistr_workflow_contracts::roadmap::Readiness;

impl RoadmapEngine {
    pub(super) fn readiness<'a>(&self, chunk: &'a Chunk) -> Readiness<'a> {
        chunk.readiness(|id| {
            self.roadmap
                .chunks
                .iter()
                .find(|c| c.id == id)
                .map(|c| c.status)
        })
    }
}
