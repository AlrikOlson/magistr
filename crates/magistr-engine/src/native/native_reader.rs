use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use magistr_workflow::roadmap::RoadmapEngine;
use magistr_workflow::roadmap::domain::{
    Chunk, FocusState, Roadmap, WorkstreamCatalog, WorkstreamFrontier, WorkstreamResolution,
    next_ready, workstream_catalog, workstream_frontier,
};
use magistr_workflow::ship::domain::check_receipt::CheckReceipt;
use magistr_workflow::ship::domain::objective::Objective;
use magistr_workflow::ship::domain::task::Task;
use magistr_workflow::ship::engine::ShipEngine;
use magistr_workflow::signal::SignalEngine;
use magistr_workflow::signal::domain::{Signal, Signals};
use magistr_workflow::think::domain::{ThinkHistory, ThinkStep};
use magistr_workflow::think::engine::ReasoningServer;

use super::native_config::NativeConfig;
use super::open_engines::open_engines;
use super::snapshot::NativeRoadmapSnapshot;
use crate::roadmap::RoadmapProgress;
use magistr_workflow_contracts::{
    NativeSnapshot, Observed, ReadError, RoadmapData, ShipData, ThinkData,
};

/// Validated queries over the four workflow owners.
/// Every observation reads the owner's actual selected source. Clones share
/// the most recent successful family values for explicitly stale snapshots;
/// direct queries never return those retained values after a failed read.
#[derive(Clone)]
pub struct NativeReader {
    project_id: String,
    /// Captured configuration used to open these owners; each observation
    /// obtains its source from the owner that performed the read.
    config: NativeConfig,
    roadmap: Arc<Mutex<RoadmapEngine>>,
    think: Arc<Mutex<ReasoningServer>>,
    ship: Arc<Mutex<ShipEngine>>,
    signal: Arc<Mutex<SignalEngine>>,
    observations: Arc<Mutex<NativeSnapshot>>,
}

fn require_current_track(catalog: &WorkstreamCatalog, track: &str) -> Result<(), String> {
    let refusal = match catalog.focus_state(Some(track)) {
        FocusState::Current { .. } => return Ok(()),
        FocusState::Closed { group } => WorkstreamResolution::Closed {
            group,
            choices: catalog.current_names(),
        },
        FocusState::Missing { .. } | FocusState::Unset => WorkstreamResolution::Unknown {
            choices: catalog.current_names(),
        },
    };
    Err(refusal.explain(track))
}

impl fmt::Debug for NativeReader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NativeReader")
            .field("project_id", &self.project_id)
            .finish_non_exhaustive()
    }
}

pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl NativeReader {
    /// Observe the project at `config.project_root`, resolving its identity
    /// the way the external server would. No lease is taken.
    #[must_use]
    pub fn open(config: &NativeConfig) -> Self {
        Self::open_for_project(config, config.resolve_project_id())
    }

    /// Observe the store under an explicit project id. No lease is taken.
    #[must_use]
    pub fn open_for_project(config: &NativeConfig, project_id: String) -> Self {
        // Event publication belongs to the admitted project writer. An
        // observer opens no publisher and never acquires a write lease.
        let (think, ship, roadmap, signal) = open_engines(config, project_id.clone(), None);
        Self::from_engines(project_id, config.clone(), think, ship, roadmap, signal)
    }

    pub(crate) fn from_engines(
        project_id: String,
        config: NativeConfig,
        think: ReasoningServer,
        ship: ShipEngine,
        roadmap: RoadmapEngine,
        signal: SignalEngine,
    ) -> Self {
        Self::from_shared(
            project_id,
            config,
            Arc::new(Mutex::new(think)),
            Arc::new(Mutex::new(ship)),
            Arc::new(Mutex::new(roadmap)),
            Arc::new(Mutex::new(signal)),
        )
    }

    /// A reader over engines something else already holds — the family
    /// services an admitted writer acts through, so its queries and its
    /// handlers look at the same engines.
    pub(crate) fn from_shared(
        project_id: String,
        config: NativeConfig,
        think: Arc<Mutex<ReasoningServer>>,
        ship: Arc<Mutex<ShipEngine>>,
        roadmap: Arc<Mutex<RoadmapEngine>>,
        signal: Arc<Mutex<SignalEngine>>,
    ) -> Self {
        Self {
            project_id,
            config,
            roadmap,
            think,
            ship,
            signal,
            observations: Arc::new(Mutex::new(NativeSnapshot::default())),
        }
    }

    /// The project every engine is keyed by.
    #[must_use]
    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    /// The store configuration the engines were opened on.
    #[must_use]
    pub fn config(&self) -> &NativeConfig {
        &self.config
    }

    /// Observe and update the retained family state while holding its owner
    /// lock. A racing clone therefore cannot replace a newer observation with
    /// an older result after the owner lock has been released.
    fn observe_roadmap(&self) -> Result<Observed<Roadmap>, ReadError> {
        super::WorkflowUpgrade::ready(&self.config, &self.project_id)?;
        let engine = lock(&self.roadmap);
        let result = engine.observe();
        let view = result.as_ref().map(|observed| Observed {
            value: RoadmapData {
                chunks: observed.value.chunks.clone(),
                progress: super::progress::progress_from_chunks(&observed.value.chunks),
            },
            source: observed.source,
        });
        lock(&self.observations)
            .roadmap
            .record(view.map_err(Clone::clone));
        result
    }

    fn observe_ship(&self) -> Result<Observed<ShipData>, ReadError> {
        super::WorkflowUpgrade::ready(&self.config, &self.project_id)?;
        let engine = lock(&self.ship);
        let result = engine.observe();
        lock(&self.observations).ship.record(result.clone());
        result
    }

    fn observe_signal(&self) -> Result<Observed<Signals>, ReadError> {
        super::WorkflowUpgrade::ready(&self.config, &self.project_id)?;
        let engine = lock(&self.signal);
        let result = engine.observe();
        let view = result.as_ref().map(|observed| Observed {
            value: observed.value.signals.clone(),
            source: observed.source,
        });
        lock(&self.observations)
            .signals
            .record(view.map_err(Clone::clone));
        result
    }

    fn observe_think(&self) -> Result<Observed<ThinkData>, ReadError> {
        super::WorkflowUpgrade::ready(&self.config, &self.project_id)?;
        let engine = lock(&self.think);
        let result = engine.observe_sessions();
        lock(&self.observations).think.record(result.clone());
        result
    }

    /// Read every family independently. Failures are part of the returned
    /// state, retaining only successfully observed records from this reader.
    #[must_use]
    pub fn snapshot(&self) -> NativeSnapshot {
        let _ = self.observe_roadmap();
        let _ = self.observe_ship();
        let _ = self.observe_signal();
        let _ = self.observe_think();
        lock(&self.observations).clone()
    }

    /// Complete sessions from the same validated owner used by snapshots and MCP.
    pub fn think_sessions(&self) -> Result<ThinkData, String> {
        Ok(self
            .observe_think()
            .map_err(|error| error.to_string())?
            .value)
    }

    /// None selects the default; a named session is resolved by the owner.
    pub fn think_session(&self, session: Option<&str>) -> Result<Option<ThinkHistory>, String> {
        super::WorkflowUpgrade::ready(&self.config, &self.project_id).map_err(|e| e.to_string())?;
        lock(&self.think)
            .observe_session(session)
            .map(|read| read.value)
            .map_err(|error| error.to_string())
    }

    /// Stable project-wide lookup across every complete session.
    pub fn think_step(&self, step_number: u32) -> Result<Option<ThinkStep>, String> {
        super::WorkflowUpgrade::ready(&self.config, &self.project_id).map_err(|e| e.to_string())?;
        lock(&self.think)
            .observe_step(step_number, false)
            .map(|read| read.value)
            .map_err(|error| error.to_string())
    }

    // ── Roadmap ────────────────────────────────────────────────────────

    /// Counts, next ready chunk, current workstreams, history and progress.
    pub fn roadmap_snapshot(&self) -> Result<NativeRoadmapSnapshot, String> {
        let observed = self.observe_roadmap().map_err(|error| error.to_string())?;
        Ok(NativeRoadmapSnapshot::from_chunks(
            &self.project_id,
            &observed.value.chunks,
        ))
    }

    /// The progress derived from one validated roadmap observation.
    pub fn roadmap_progress(&self) -> Result<RoadmapProgress, String> {
        let observed = self.observe_roadmap().map_err(|error| error.to_string())?;
        Ok(super::progress::progress_from_chunks(
            &observed.value.chunks,
        ))
    }

    /// The next ready chunk, including ungrouped work.
    pub fn roadmap_next(&self) -> Result<Option<Chunk>, String> {
        let observed = self.observe_roadmap().map_err(|error| error.to_string())?;
        let chunks = &observed.value.chunks;
        Ok(next_ready(
            chunks,
            |chunk| {
                chunk
                    .readiness(|id| {
                        chunks
                            .iter()
                            .find(|candidate| candidate.id == id)
                            .map(|candidate| candidate.status)
                    })
                    .is_ready()
            },
            |chunk| chunk.priority,
        )
        .cloned())
    }

    /// The next ready chunk in an exactly named current workstream.
    /// Closed or missing saved names never select similarly named work.
    pub fn roadmap_next_in_track(&self, track: &str) -> Result<Option<Chunk>, String> {
        let observed = self.observe_roadmap().map_err(|error| error.to_string())?;
        let chunks = &observed.value.chunks;
        require_current_track(&workstream_catalog(chunks), track)?;
        Ok(next_ready(
            chunks,
            |chunk| {
                chunk.group.as_deref() == Some(track)
                    && chunk
                        .readiness(|id| {
                            chunks
                                .iter()
                                .find(|candidate| candidate.id == id)
                                .map(|candidate| candidate.status)
                        })
                        .is_ready()
            },
            |chunk| chunk.priority,
        )
        .cloned())
    }

    /// Current workstreams, authored history and ungrouped work from one read.
    pub fn roadmap_workstreams(&self) -> Result<WorkstreamCatalog, String> {
        let observed = self.observe_roadmap().map_err(|error| error.to_string())?;
        Ok(workstream_catalog(&observed.value.chunks))
    }

    /// Current work in an exact workstream; None selects ungrouped work.
    pub fn roadmap_frontier(&self, group: Option<&str>) -> Result<WorkstreamFrontier, String> {
        let observed = self.observe_roadmap().map_err(|error| error.to_string())?;
        let chunks = &observed.value.chunks;
        if let Some(group) = group {
            require_current_track(&workstream_catalog(chunks), group)?;
        }
        Ok(workstream_frontier(chunks, group))
    }

    /// Resolve an explicit current name or unambiguous fragment without writing.
    /// An exact authored closed name is a refusal before fragment matching.
    pub fn roadmap_resolve_track(&self, query: &str) -> Result<String, String> {
        match self.roadmap_workstreams()?.resolve_current(query) {
            WorkstreamResolution::Current { group } => Ok(group),
            refusal => Err(refusal.explain(query)),
        }
    }

    /// One current chunk by id.
    pub fn roadmap_chunk(&self, id: &str) -> Result<Option<Chunk>, String> {
        Ok(self
            .roadmap_chunks()?
            .into_iter()
            .find(|chunk| chunk.id == id))
    }

    /// Every chunk in stored order, after a successful observation.
    pub fn roadmap_chunks(&self) -> Result<Vec<Chunk>, String> {
        Ok(self
            .observe_roadmap()
            .map_err(|error| error.to_string())?
            .value
            .chunks)
    }

    /// The owner's bounded status projection of validated records.
    pub fn roadmap_status(&self) -> Result<serde_json::Value, String> {
        let observed = self.observe_roadmap().map_err(|error| error.to_string())?;
        Ok(RoadmapEngine::status_of(&observed.value))
    }

    /// The owner's markdown or JSON projection of validated records.
    pub fn roadmap_export(&self, format: &str) -> Result<String, String> {
        let observed = self.observe_roadmap().map_err(|error| error.to_string())?;
        Ok(RoadmapEngine::export_of(&observed.value, format))
    }

    // ── Ship ───────────────────────────────────────────────────────────

    /// The objective and complete plan from one successful owner read.
    pub fn ship_snapshot(&self) -> Result<ShipData, String> {
        Ok(self
            .observe_ship()
            .map_err(|error| error.to_string())?
            .value)
    }

    /// Resolve a selected or retained execution by its stable run identity.
    /// Missing work is distinct from a failed owner read.
    pub fn ship_run(
        &self,
        run_id: &str,
    ) -> Result<Option<magistr_workflow_contracts::ShipRun>, String> {
        Ok(self.ship_snapshot()?.run(run_id).cloned())
    }

    pub fn ship_objective(&self) -> Result<Option<Objective>, String> {
        Ok(self.ship_snapshot()?.current.map(|run| run.objective))
    }

    pub fn ship_active_task_id(&self) -> Result<Option<String>, String> {
        Ok(self
            .ship_snapshot()?
            .active_task()
            .map(|task| task.id.clone()))
    }

    /// Read the exact durable receipt through its own ledger. Its authority
    /// does not depend on whether the separate execution summary is readable.
    pub fn ship_receipt(&self, id: &str) -> Result<Option<CheckReceipt>, String> {
        let ledger = lock(&self.ship).receipts().clone();
        ledger.get(id).map_err(|error| error.to_string())
    }

    pub fn ship_tasks(&self) -> Result<Vec<Task>, String> {
        Ok(self
            .ship_snapshot()?
            .current
            .map_or_else(Vec::new, |run| run.tasks))
    }

    // ── Signal ─────────────────────────────────────────────────────────

    /// The owner's bounded inbox projection of validated records.
    pub fn signal_status(&self) -> Result<serde_json::Value, String> {
        let observed = self.observe_signal().map_err(|error| error.to_string())?;
        Ok(
            serde_json::to_value(magistr_workflow::signal::application::SignalInbox::from(
                &observed.value,
            ))
            .expect("signal inbox contains serializable domain fields"),
        )
    }

    pub fn signal(&self, id: &str) -> Result<Option<Signal>, String> {
        Ok(self
            .signal_list()?
            .into_iter()
            .find(|signal| signal.id == id))
    }

    pub fn signal_list(&self) -> Result<Vec<Signal>, String> {
        Ok(self
            .observe_signal()
            .map_err(|error| error.to_string())?
            .value
            .signals)
    }
}

#[cfg(test)]
#[path = "native_reader.tests.rs"]
mod tests;
