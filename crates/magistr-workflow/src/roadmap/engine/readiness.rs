//! Scheduling policy is shared; the writer supplies current records and names.
use super::{Chunk, ChunkStatus, RoadmapEngine};
use magistr_workflow_contracts::roadmap::{Readiness, WaitingDependency};

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

    pub(super) fn waiting_reason(&self, deps: &[WaitingDependency<'_>]) -> String {
        format!(
            "Waiting for {}",
            deps.iter()
                .map(|dep| {
                    self.roadmap
                        .chunks
                        .iter()
                        .find(|c| c.id == dep.id)
                        .map_or("an unavailable work item", |c| c.name.as_str())
                })
                .collect::<Vec<_>>()
                .join(", ")
        )
    }

    pub(super) fn require_ready(&self, chunk: &Chunk) -> Result<(), String> {
        let reason = match self.readiness(chunk) {
            Readiness::Ready => return Ok(()),
            Readiness::Blocked(reason) => reason.to_owned(),
            Readiness::Waiting(deps) => self.waiting_reason(&deps),
            Readiness::Inactive(status) => match status {
                ChunkStatus::Backlog => "This work item is planned for later.",
                ChunkStatus::InProgress => "Work is already underway.",
                ChunkStatus::Blocked => {
                    "Resolve this work item's blocker and return it to the work queue."
                }
                ChunkStatus::Done => "This work item is already completed.",
                ChunkStatus::Obsoleted => "This work item has been replaced.",
                ChunkStatus::Pending => "This work item is waiting.",
            }
            .into(),
        };
        Err(format!("Cannot start {}: {reason}", chunk.name))
    }
}
