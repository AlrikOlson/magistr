use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use think_and_ship::roadmap::RoadmapEngine;
use think_and_ship::roadmap::domain::ChunkStatus;

use super::progress::progress_from_roadmap;
use super::region_counts::RegionCounts;
use crate::roadmap::RoadmapProgress;

/// What the engine surfaces about a think-and-ship roadmap: status counts,
/// the next ready chunk, what is in flight, and per-workstream (region)
/// counts. Small on purpose — a view model, not the roadmap.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NativeRoadmapSnapshot {
    pub project_id: String,
    /// Chunk counts keyed by status wire name (`pending`, `done`, …).
    pub counts: BTreeMap<String, u32>,
    /// The chunk `roadmap_next` would return.
    pub next_ready: Option<String>,
    /// Ids currently `in_progress`.
    pub in_progress: Vec<String>,
    /// Workstream → counts. Ungrouped chunks are not a region.
    pub regions: BTreeMap<String, RegionCounts>,
    pub progress: RoadmapProgress,
}

impl NativeRoadmapSnapshot {
    /// Build from a live engine (the only place `next()` can be answered,
    /// since dependency satisfaction is the engine's rule).
    #[must_use]
    pub fn from_engine(engine: &RoadmapEngine) -> Self {
        let roadmap = engine.roadmap();
        let mut counts: BTreeMap<String, u32> = BTreeMap::new();
        let mut regions: BTreeMap<String, RegionCounts> = BTreeMap::new();
        let mut in_progress = Vec::new();
        for chunk in &roadmap.chunks {
            *counts
                .entry(status_key(chunk.status).to_owned())
                .or_insert(0) += 1;
            if chunk.status == ChunkStatus::Obsoleted {
                continue;
            }
            if chunk.status == ChunkStatus::InProgress {
                in_progress.push(chunk.id.clone());
            }
            if let Some(group) = &chunk.group {
                let r = regions.entry(group.clone()).or_default();
                r.total += 1;
                match chunk.status {
                    ChunkStatus::Backlog => r.backlog += 1,
                    ChunkStatus::Pending => r.pending += 1,
                    ChunkStatus::InProgress => r.in_progress += 1,
                    ChunkStatus::Blocked => r.blocked += 1,
                    ChunkStatus::Done => r.done += 1,
                    ChunkStatus::Obsoleted => {}
                }
            }
        }
        Self {
            project_id: roadmap.project_id.clone(),
            counts,
            next_ready: engine.next().map(|c| c.id.clone()),
            in_progress,
            regions,
            progress: progress_from_roadmap(roadmap),
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
