use std::collections::BTreeMap;

use magistr_workflow::roadmap::domain::{
    Chunk, ChunkStatus, WorkstreamCatalog, next_ready, workstream_catalog,
};
use serde::{Deserialize, Serialize};

use super::progress::progress_from_chunks;
use crate::roadmap::RoadmapProgress;

/// What the engine surfaces about a magistr roadmap: status counts,
/// the next ready chunk, what is in flight, and the canonical current and
/// historical workstreams. Small on purpose — a view model, not the roadmap.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NativeRoadmapSnapshot {
    pub project_id: String,
    /// Chunk counts keyed by status wire name (`pending`, `done`, …).
    pub counts: BTreeMap<String, u32>,
    /// The chunk `roadmap_next` would return.
    pub next_ready: Option<String>,
    /// Ids currently `in_progress`.
    pub in_progress: Vec<String>,
    /// Current work, authored history and ungrouped work from the roadmap owner.
    pub workstreams: WorkstreamCatalog,
    pub progress: RoadmapProgress,
}

impl NativeRoadmapSnapshot {
    /// Project one roadmap observation using the owner's readiness rules.
    #[must_use]
    pub fn from_chunks(project_id: &str, chunks: &[Chunk]) -> Self {
        let mut counts: BTreeMap<String, u32> = BTreeMap::new();
        let mut in_progress = Vec::new();
        for chunk in chunks {
            *counts
                .entry(status_key(chunk.status).to_owned())
                .or_insert(0) += 1;
            if chunk.status == ChunkStatus::Obsoleted {
                continue;
            }
            if chunk.status == ChunkStatus::InProgress {
                in_progress.push(chunk.id.clone());
            }
        }
        Self {
            project_id: project_id.to_owned(),
            counts,
            next_ready: next_ready(
                chunks,
                |chunk| {
                    chunk
                        .readiness(|id| chunks.iter().find(|c| c.id == id).map(|c| c.status))
                        .is_ready()
                },
                |chunk| chunk.priority,
            )
            .map(|chunk| chunk.id.clone()),
            in_progress,
            workstreams: workstream_catalog(chunks),
            progress: progress_from_chunks(chunks),
        }
    }

    /// Count for one status wire name.
    #[must_use]
    pub fn count(&self, status: &str) -> u32 {
        self.counts.get(status).copied().unwrap_or(0)
    }
}

const fn status_key(status: ChunkStatus) -> &'static str {
    match status {
        ChunkStatus::Backlog => "backlog",
        ChunkStatus::Pending => "pending",
        ChunkStatus::InProgress => "in_progress",
        ChunkStatus::Blocked => "blocked",
        ChunkStatus::Done => "done",
        ChunkStatus::Obsoleted => "obsoleted",
    }
}

#[cfg(test)]
#[path = "snapshot.tests.rs"]
mod tests;
