use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use magistr_workflow::roadmap::RoadmapEngine;
use magistr_workflow::roadmap::domain::Chunk;
use magistr_workflow::ship::domain::check_receipt::CheckReceipt;
use magistr_workflow::ship::domain::objective::Objective;
use magistr_workflow::ship::domain::task::Task;
use magistr_workflow::ship::engine::ShipEngine;
use magistr_workflow::signal::SignalEngine;
use magistr_workflow::signal::domain::Signal;
use magistr_workflow::think::domain::ThinkStep;
use magistr_workflow::think::engine::ReasoningServer;

use super::native_config::NativeConfig;
use super::native_snapshot::NativeSnapshot;
use super::open_engines::open_engines;
use super::snapshot::NativeRoadmapSnapshot;
use super::snapshot_cursor::SnapshotCursor;
use crate::roadmap::RoadmapProgress;

/// The read capability over the four think-and-ship engines: queries,
/// reloads and the store stamp. It needs no admission — the store is the
/// library's locked merge-on-save file, and every query observes it
/// first: a `stat` of the family's file against the stamp the engine last
/// read or wrote, and a re-read only when it moved — so a `magistr roadmap
/// status`, a desktop projection or the cockpit's re-seed timer runs
/// beside any admitted writer and answers what that writer persisted.
///
/// Cheap to clone (every engine sits behind an `Arc<Mutex<_>>`); `Send +
/// Sync`. Each query takes the one lock it needs and releases it before
/// returning. The write capability, [`super::NativeEngines`], derefs to this.
#[derive(Clone)]
pub struct NativeReader {
    project_id: String,
    /// The store these engines were opened on — what a reload re-reads.
    config: NativeConfig,
    roadmap: Arc<Mutex<RoadmapEngine>>,
    think: Arc<Mutex<ReasoningServer>>,
    ship: Arc<Mutex<ShipEngine>>,
    signal: Arc<Mutex<SignalEngine>>,
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

    /// Re-read the roadmap from the store whatever its stamp says. Every
    /// query already observes the store, so this is for a caller that
    /// wants the read to happen now — the loop at an iteration boundary,
    /// the cockpit's re-seed. Silent: the engine keeps its wiring and
    /// re-reads its file, nothing is re-opened and no load banner prints.
    pub fn reload_roadmap(&self) {
        if let Err(error) = lock(&self.roadmap).reload_from_store() {
            tracing::warn!("magistr: roadmap store not re-read: {error}");
        }
    }

    /// Re-read the execution trace from the store; see
    /// [`Self::reload_roadmap`]. Silent as well.
    pub fn reload_ship(&self) {
        if let Err(error) = lock(&self.ship).reload_from_store() {
            tracing::warn!("magistr: ship store not re-read: {error}");
        }
    }

    /// Re-read the signal inbox from the store; see
    /// [`Self::reload_roadmap`]. Silent as well.
    pub fn reload_signal(&self) {
        if let Err(error) = lock(&self.signal).reload_from_store() {
            tracing::warn!("magistr: signal store not re-read: {error}");
        }
    }

    /// The roadmap engine, observed: re-read when its file's stamp moved
    /// since it was last read or written, else as it stands. A store that
    /// cannot be observed is logged and the engine answers what it holds —
    /// a query has no refusal to give.
    fn current_roadmap(&self) -> MutexGuard<'_, RoadmapEngine> {
        let mut engine = lock(&self.roadmap);
        if let Err(error) = engine.refresh_if_stale() {
            tracing::warn!("magistr: roadmap store not observed: {error}");
        }
        engine
    }

    /// The ship engine, observed; see [`Self::current_roadmap`].
    fn current_ship(&self) -> MutexGuard<'_, ShipEngine> {
        let mut engine = lock(&self.ship);
        if let Err(error) = engine.refresh_if_stale() {
            tracing::warn!("magistr: ship store not observed: {error}");
        }
        engine
    }

    /// The signal engine, observed; see [`Self::current_roadmap`].
    fn current_signal(&self) -> MutexGuard<'_, SignalEngine> {
        let mut engine = lock(&self.signal);
        if let Err(error) = engine.refresh_if_stale() {
            tracing::warn!("magistr: signal store not observed: {error}");
        }
        engine
    }

    /// Where the store stands on disk: the newest modification time across
    /// the persisted domains and their total length. `None` with
    /// persistence off, and while nothing has been written yet.
    ///
    /// A cheap "did anyone else touch this?" for a host that re-reads the
    /// store on a timer (the cockpit's re-seed while no feed is attached):
    /// a reload is not free (and [`Self::reload_ship`] is not silent), so a
    /// caller compares cursors and reloads only when one moved. See
    /// [`SnapshotCursor`] for what it is not.
    #[must_use]
    pub fn snapshot_cursor(&self) -> Option<SnapshotCursor> {
        use magistr_workflow::infra::{Domain, Persistence};

        let persist = self.config.persistence_config();
        if !persist.enabled {
            return None;
        }
        let mut files: Vec<std::path::PathBuf> = [Domain::Roadmap, Domain::Ship, Domain::Signal]
            .into_iter()
            .map(|domain| Persistence::new(&persist, domain).path_for(&self.project_id))
            .collect();
        files.push(
            magistr_workflow::think::persistence::Persistence::for_project(
                &persist,
                &self.project_id,
            )
            .default_store_path(),
        );
        files
            .iter()
            .filter_map(|path| std::fs::metadata(path).ok())
            .filter_map(|meta| {
                Some(SnapshotCursor {
                    modified: meta.modified().ok()?,
                    length: meta.len(),
                })
            })
            .reduce(|newest, next| SnapshotCursor {
                modified: newest.modified.max(next.modified),
                length: newest.length.saturating_add(next.length),
            })
    }

    /// The store read whole, family by family, as [`NativeSnapshot`]
    /// documents it: the cursor first, then the ship objective and tasks,
    /// the roadmap's chunks and progress, the signals, and the persisted
    /// reasoning steps and branch metadata — each family observed and read
    /// under its own lock, none of them under a shared one.
    #[must_use]
    pub fn snapshot(&self) -> NativeSnapshot {
        let cursor = self.snapshot_cursor();
        let (objective, tasks) = {
            let ship = self.current_ship();
            (ship.objective.clone(), ship.tasks.clone())
        };
        let (chunks, progress) = {
            let roadmap = self.current_roadmap();
            (
                roadmap.roadmap().chunks.clone(),
                super::progress::progress_from_roadmap(roadmap.roadmap()),
            )
        };
        let signals = self.current_signal().signals().signals.clone();
        let (steps, branches) = self.think_history_from_store().map_or_else(
            || (Vec::new(), Vec::new()),
            |history| {
                let branches = history
                    .branches
                    .as_deref()
                    .unwrap_or_default()
                    .iter()
                    .map(super::ThinkBranch::from)
                    .collect();
                (history.steps, branches)
            },
        );
        NativeSnapshot {
            cursor,
            objective,
            tasks,
            progress,
            chunks,
            signals,
            steps,
            branches,
        }
    }

    /// The reasoning steps as the store holds them right now, read silently
    /// from the archive another process (the agent's `magistr mcp-serve`) is
    /// writing. The reasoning server is deliberately not re-opened: its load
    /// path can print (legacy-history adoption), which a raw-mode terminal
    /// must never see. Empty when persistence is off.
    #[must_use]
    pub fn think_steps_from_store(&self) -> Vec<ThinkStep> {
        self.think_history_from_store()
            .map(|history| history.steps)
            .unwrap_or_default()
    }

    fn think_history_from_store(&self) -> Option<magistr_workflow::think::domain::ThinkHistory> {
        use magistr_workflow::think::persistence::{Persistence, read_history};
        let persist = self.config.persistence_config();
        if !persist.enabled {
            return None;
        }
        let store = Persistence::for_project(&persist, &self.project_id);
        read_history(&store.default_store_path())
    }

    /// The pinned reasoning steps of the live trace, in step order.
    #[must_use]
    pub fn think_pinned_steps(&self) -> Vec<serde_json::Value> {
        lock(&self.think).pinned_steps()
    }

    /// One step of the live trace by number.
    #[must_use]
    pub fn think_step(&self, step_number: u32) -> Option<ThinkStep> {
        lock(&self.think).step_by_number(step_number)
    }

    // ── Roadmap ────────────────────────────────────────────────────────

    /// Counts, next ready chunk, regions and progress.
    #[must_use]
    pub fn roadmap_snapshot(&self) -> NativeRoadmapSnapshot {
        NativeRoadmapSnapshot::from_engine(&self.current_roadmap())
    }

    /// The engine's progress view of the roadmap.
    #[must_use]
    pub fn roadmap_progress(&self) -> RoadmapProgress {
        super::progress::progress_from_roadmap(self.current_roadmap().roadmap())
    }

    /// The next ready chunk, if any.
    #[must_use]
    pub fn roadmap_next(&self) -> Option<Chunk> {
        self.current_roadmap().next().cloned()
    }

    /// The next ready chunk in one track; never falls through to another.
    #[must_use]
    pub fn roadmap_next_in_track(&self, track: &str) -> Option<Chunk> {
        self.current_roadmap().next_in_group(track).cloned()
    }

    /// Track names as the roadmap owner stores them.
    #[must_use]
    pub fn roadmap_tracks(&self) -> Vec<String> {
        self.current_roadmap().groups()
    }

    /// Resolve an exact track name or an unambiguous fragment without writing.
    pub fn roadmap_resolve_track(&self, query: &str) -> Result<String, String> {
        match self.current_roadmap().resolve_group(query) {
            magistr_workflow::roadmap::engine::GroupResolution::Exact(name) => Ok(name),
            refusal => Err(refusal.explain(query)),
        }
    }

    /// One chunk by id.
    #[must_use]
    pub fn roadmap_chunk(&self, id: &str) -> Option<Chunk> {
        self.current_roadmap()
            .roadmap()
            .chunks
            .iter()
            .find(|c| c.id == id)
            .cloned()
    }

    /// Every chunk, in stored order.
    #[must_use]
    pub fn roadmap_chunks(&self) -> Vec<Chunk> {
        self.current_roadmap().roadmap().chunks.clone()
    }

    /// The same bounded JSON `roadmap_status` answers over MCP.
    #[must_use]
    pub fn roadmap_status(&self) -> serde_json::Value {
        self.current_roadmap().status()
    }

    /// The markdown (or json) projection `roadmap_export` produces.
    #[must_use]
    pub fn roadmap_export(&self, format: &str) -> String {
        self.current_roadmap().export(format)
    }

    // ── Ship ───────────────────────────────────────────────────────────

    /// The full execution-state snapshot.
    #[must_use]
    pub fn ship_status(&self) -> serde_json::Value {
        self.current_ship().status()
    }

    /// The ship objective, once one is set.
    #[must_use]
    pub fn ship_objective(&self) -> Option<Objective> {
        self.current_ship().objective.clone()
    }

    /// The task a record with no explicit task would land on — the one
    /// started and not yet closed — so a caller can name it rather than
    /// rely on "whichever is active" at write time.
    #[must_use]
    pub fn ship_active_task_id(&self) -> Option<String> {
        self.current_ship().active_task_id()
    }

    /// One durable check receipt by its id, as the ship ledger holds it
    /// now — the exact admission (run, task instance, attempt, gate) and,
    /// once settled, how the command ended and whether the workflow
    /// applied the result or kept it stale. `Ok(None)` for an id the
    /// ledger never saw; `Err` when the ledger cannot be read.
    ///
    /// # Errors
    /// The receipt store cannot be read.
    pub fn ship_receipt(&self, id: &str) -> Result<Option<CheckReceipt>, String> {
        let ledger = self.current_ship().receipts().clone();
        ledger.get(id)
    }

    /// Every task in the execution trace, in plan order, with its actions
    /// and checks whole — what [`Self::ship_status`] cannot give: its
    /// `recent_actions` is capped at five and drops the task each action
    /// belongs to, and its `checks` drop the exit code and the duration.
    /// The TUI's trace seed reads the timeline off this.
    #[must_use]
    pub fn ship_tasks(&self) -> Vec<Task> {
        self.current_ship().tasks.clone()
    }

    // ── Signal ─────────────────────────────────────────────────────────

    /// Signal status counts.
    #[must_use]
    pub fn signal_status(&self) -> serde_json::Value {
        self.current_signal().status()
    }

    /// One signal by id.
    #[must_use]
    pub fn signal(&self, id: &str) -> Option<Signal> {
        self.current_signal().get(id).ok().cloned()
    }

    /// Every signal in the store, capture order.
    #[must_use]
    pub fn signal_list(&self) -> Vec<Signal> {
        self.current_signal().signals().signals.clone()
    }
}
