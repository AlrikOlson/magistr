//! Each workflow family's records and observation state form one view.
//! Reads are coherent within a family, never a transaction across families.

use serde::{Deserialize, Serialize};

use crate::roadmap::{Chunk, WorkstreamCatalog, workstream_catalog};
use crate::ship::{Objective, Task, TaskStatus};
use crate::signal::Signal;
use crate::think::ThinkStep;
use crate::{ReadError, ReadState, RoadmapProgress, ThinkBranch};

/// Roadmap records and progress derived from the same validated observation.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RoadmapData {
    pub chunks: Vec<Chunk>,
    pub progress: RoadmapProgress,
}

/// The execution plan as it was observed, without inventing a missing run.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ShipData {
    pub objective: Option<Objective>,
    pub tasks: Vec<Task>,
}

impl ShipData {
    #[must_use]
    pub fn active_task(&self) -> Option<&Task> {
        self.tasks
            .iter()
            .find(|task| task.status == TaskStatus::Active)
    }
}

/// Reasoning steps and branch metadata from the owner's selected session.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ThinkData {
    /// The selected session; None only when reading the unscoped default archive.
    pub session_id: Option<String>,
    pub steps: Vec<ThinkStep>,
    pub branches: Vec<ThinkBranch>,
}

/// Independent observations from the four workflow owners.
/// A failed family retains its last successful data explicitly as stale.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NativeSnapshot {
    pub roadmap: ReadState<RoadmapData>,
    pub ship: ReadState<ShipData>,
    pub signals: ReadState<Vec<Signal>>,
    pub think: ReadState<ThinkData>,
}

impl NativeSnapshot {
    /// Derivation preserves the roadmap's read state and provenance.
    #[must_use]
    pub fn workstreams(&self) -> ReadState<WorkstreamCatalog> {
        self.roadmap
            .map(|roadmap| workstream_catalog(&roadmap.chunks))
    }

    #[must_use]
    pub fn needs_retry(&self) -> bool {
        self.roadmap.needs_retry()
            || self.ship.needs_retry()
            || self.signals.needs_retry()
            || self.think.needs_retry()
    }

    /// A worker failed before it could report individual family observations.
    /// Successful values remain visible as stale; unread families stay unavailable.
    #[must_use]
    pub fn failed(mut self, error: ReadError) -> Self {
        self.roadmap.record(Err(error.clone()));
        self.ship.record(Err(error.clone()));
        self.signals.record(Err(error.clone()));
        self.think.record(Err(error));
        self
    }
}

#[cfg(test)]
#[path = "native_snapshot.tests.rs"]
mod tests;
