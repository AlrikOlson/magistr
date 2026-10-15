//! Each workflow family's records and observation state form one view.
//! Reads are coherent within a family, never a transaction across families.

use serde::{Deserialize, Serialize};

use crate::roadmap::{Chunk, WorkstreamCatalog, workstream_catalog};
use crate::ship::{Objective, Task, TaskStatus};
use crate::signal::Signal;
use crate::think::ThinkHistory;
use crate::{ReadError, ReadState, RoadmapProgress};

/// Roadmap records and progress derived from the same validated observation.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RoadmapData {
    pub chunks: Vec<Chunk>,
    pub progress: RoadmapProgress,
}

/// A complete authored execution, addressed independently of reusable local task names.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShipRun {
    pub run_id: String,
    pub task_instances: std::collections::BTreeMap<String, String>,
    pub objective: Objective,
    pub tasks: Vec<Task>,
}

/// Selected and retained executions from the same validated owner observation.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ShipData {
    pub current: Option<ShipRun>,
    pub retained: Vec<ShipRun>,
}

impl ShipData {
    #[must_use]
    pub fn objective(&self) -> Option<&Objective> {
        self.current.as_ref().map(|run| &run.objective)
    }

    #[must_use]
    pub fn tasks(&self) -> &[Task] {
        self.current
            .as_ref()
            .map_or(&[], |run| run.tasks.as_slice())
    }

    #[must_use]
    pub fn active_task(&self) -> Option<&Task> {
        self.tasks()
            .iter()
            .find(|task| task.status == TaskStatus::Active)
    }

    #[must_use]
    pub fn run(&self, run_id: &str) -> Option<&ShipRun> {
        self.current
            .iter()
            .chain(&self.retained)
            .find(|run| run.run_id == run_id)
    }
}

/// Complete reasoning sessions from one validated project observation.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ThinkData {
    pub sessions: Vec<ThinkHistory>,
}

impl ThinkData {
    /// None addresses the default session. Named identities are canonical.
    pub fn session(&self, id: Option<&str>) -> Option<&ThinkHistory> {
        self.sessions
            .iter()
            .find(|history| history.session_id.as_deref() == id)
    }
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
