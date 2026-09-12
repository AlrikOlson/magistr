use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use magistr_workflow::infra::resolve_project_id_with;
use magistr_workflow::roadmap::RoadmapEngine;
use magistr_workflow::roadmap::domain::Chunk;
use magistr_workflow::ship::domain::objective::Objective;
use magistr_workflow::ship::domain::task::Task;
use magistr_workflow::ship::engine::ShipEngine;
use magistr_workflow::signal::SignalEngine;
use magistr_workflow::signal::domain::Signal;
use magistr_workflow::think::domain::ThinkStep;
use magistr_workflow::think::engine::ReasoningServer;

use super::native_config::NativeConfig;
use super::native_snapshot::NativeSnapshot;
use super::open_engines::{open_engines, open_roadmap, open_ship, open_signal};
use super::snapshot::NativeRoadmapSnapshot;
use super::snapshot_cursor::SnapshotCursor;
use crate::roadmap::RoadmapProgress;

/// The read capability over the four think-and-ship engines: queries,
/// reloads and the store stamp. It needs no admission — the store is the
/// library's locked merge-on-save file, and a reader re-reads it when the
/// stamp moves — so a `magistr roadmap status`, a desktop projection or the
/// cockpit's re-seed timer runs beside any admitted writer.
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
        let (project_id, _) = resolve_project_id_with(None, Some(config.project_root()));
        Self::open_for_project(config, project_id)
    }

    /// Observe the store under an explicit project id. No lease is taken.
    #[must_use]
    pub fn open_for_project(config: &NativeConfig, project_id: String) -> Self {
        // The socket, if any, belongs to the publishing service; a host
        // subscribes to it (see `spawn_broadcast_subscriber`) rather than
        // bind a second.
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

    /// Re-read the roadmap from the store, picking up writes another
    /// process (the agent's `magistr mcp-serve`) made since this instance
    /// was opened. Quiet since think-and-ship 0.5.1: the load narrates on
    /// `tracing::info!` (see [`open_roadmap`]), so under the default `warn`
    /// subscriber nothing reaches stderr — the loop driver reaches this at
    /// every iteration boundary through `native_spec_summary` and prints
    /// nothing for it.
    pub fn reload_roadmap(&self) {
        *lock(&self.roadmap) = open_roadmap(&self.config, self.project_id.clone());
    }

    /// Re-read the execution trace from the store; see [`Self::reload_roadmap`]
    /// — but unlike it, not quiet: through 0.5.2 the ship load `eprintln!`s
    /// its task count (see [`open_ship`]), so a caller inside a raw-mode
    /// terminal must own stderr for the session, as the cockpit does.
    pub fn reload_ship(&self) {
        *lock(&self.ship) = open_ship(&self.config, self.project_id.clone());
    }

    /// Re-read the signal inbox from the store; see [`Self::reload_ship`],
    /// whose stderr caveat this shares — the signal load path still
    /// `eprintln!`s its count (see [`open_signal`]).
    ///
    /// The one domain a host cannot get at any other way: the roadmap and
    /// the execution trace have their reloads above and the reasoning
    /// trace is read straight off disk by
    /// [`Self::think_steps_from_store`], but [`Self::signal_list`] and
    /// [`Self::signal_status`] answer from the engine opened at startup.
    /// Without this a cockpit re-seeding on a timer shows the inbox it
    /// booted with, however many signals the agent's `magistr mcp-serve`
    /// captured since.
    pub fn reload_signal(&self) {
        *lock(&self.signal) = open_signal(&self.config, self.project_id.clone());
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
    /// reasoning steps — each family under its own lock, none of them
    /// under a shared one.
    #[must_use]
    pub fn snapshot(&self) -> NativeSnapshot {
        let cursor = self.snapshot_cursor();
        let (objective, tasks) = {
            let ship = lock(&self.ship);
            (ship.objective.clone(), ship.tasks.clone())
        };
        let (chunks, progress) = {
            let roadmap = lock(&self.roadmap);
            (
                roadmap.roadmap().chunks.clone(),
                super::progress::progress_from_roadmap(roadmap.roadmap()),
            )
        };
        let signals = lock(&self.signal).signals().signals.clone();
        NativeSnapshot {
            cursor,
            objective,
            tasks,
            progress,
            chunks,
            signals,
            steps: self.think_steps_from_store(),
        }
    }

    /// The reasoning steps as the store holds them right now, read silently
    /// from the archive another process (the agent's `magistr mcp-serve`) is
    /// writing. The reasoning server is deliberately not re-opened: its load
    /// path can print (legacy-history adoption), which a raw-mode terminal
    /// must never see. Empty when persistence is off.
    #[must_use]
    pub fn think_steps_from_store(&self) -> Vec<ThinkStep> {
        use magistr_workflow::think::persistence::{Persistence, read_history};
        let persist = self.config.persistence_config();
        if !persist.enabled {
            return Vec::new();
        }
        let store = Persistence::for_project(&persist, &self.project_id);
        read_history(&store.default_store_path())
            .map(|history| history.steps)
            .unwrap_or_default()
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
        NativeRoadmapSnapshot::from_engine(&lock(&self.roadmap))
    }

    /// The engine's progress view of the roadmap.
    #[must_use]
    pub fn roadmap_progress(&self) -> RoadmapProgress {
        super::progress::progress_from_roadmap(lock(&self.roadmap).roadmap())
    }

    /// The next ready chunk, if any.
    #[must_use]
    pub fn roadmap_next(&self) -> Option<Chunk> {
        lock(&self.roadmap).next().cloned()
    }

    /// One chunk by id.
    #[must_use]
    pub fn roadmap_chunk(&self, id: &str) -> Option<Chunk> {
        lock(&self.roadmap)
            .roadmap()
            .chunks
            .iter()
            .find(|c| c.id == id)
            .cloned()
    }

    /// Every chunk, in stored order.
    #[must_use]
    pub fn roadmap_chunks(&self) -> Vec<Chunk> {
        lock(&self.roadmap).roadmap().chunks.clone()
    }

    /// The same bounded JSON `roadmap_status` answers over MCP.
    #[must_use]
    pub fn roadmap_status(&self) -> serde_json::Value {
        lock(&self.roadmap).status()
    }

    /// The markdown (or json) projection `roadmap_export` produces.
    #[must_use]
    pub fn roadmap_export(&self, format: &str) -> String {
        lock(&self.roadmap).export(format)
    }

    // ── Ship ───────────────────────────────────────────────────────────

    /// The full execution-state snapshot.
    #[must_use]
    pub fn ship_status(&self) -> serde_json::Value {
        lock(&self.ship).status()
    }

    /// The ship objective, once one is set.
    #[must_use]
    pub fn ship_objective(&self) -> Option<Objective> {
        lock(&self.ship).objective.clone()
    }

    /// The task a record with no explicit task would land on — the one
    /// started and not yet closed — so a caller can name it rather than
    /// rely on "whichever is active" at write time.
    #[must_use]
    pub fn ship_active_task_id(&self) -> Option<String> {
        lock(&self.ship).active_task_id()
    }

    /// Every task in the execution trace, in plan order, with its actions
    /// and checks whole — what [`Self::ship_status`] cannot give: its
    /// `recent_actions` is capped at five and drops the task each action
    /// belongs to, and its `checks` drop the exit code and the duration.
    /// The TUI's trace seed reads the timeline off this.
    #[must_use]
    pub fn ship_tasks(&self) -> Vec<Task> {
        lock(&self.ship).tasks.clone()
    }

    // ── Signal ─────────────────────────────────────────────────────────

    /// Signal status counts.
    #[must_use]
    pub fn signal_status(&self) -> serde_json::Value {
        lock(&self.signal).status()
    }

    /// One signal by id.
    #[must_use]
    pub fn signal(&self, id: &str) -> Option<Signal> {
        lock(&self.signal).get(id).ok().cloned()
    }

    /// Every signal in the store, capture order.
    #[must_use]
    pub fn signal_list(&self) -> Vec<Signal> {
        lock(&self.signal).signals().signals.clone()
    }
}
