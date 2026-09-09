use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use think_and_ship::content::StructuredContent;
use think_and_ship::infra::resolve_project_id_with;
use think_and_ship::roadmap::RoadmapEngine;
use think_and_ship::roadmap::domain::{BlockerKind, Chunk, ChunkStatus};
use think_and_ship::ship::domain::action::{Action, ActionType};
use think_and_ship::ship::domain::artifact::Artifact;
use think_and_ship::ship::domain::check::{Check, CheckType};
use think_and_ship::ship::domain::task::{Task, TaskType};
use think_and_ship::ship::engine::ShipEngine;
use think_and_ship::signal::SignalEngine;
use think_and_ship::signal::domain::{Signal, SignalKind};
use think_and_ship::think::domain::ThinkStep;
use think_and_ship::think::engine::{ProcessResult, ReasoningServer};

use super::open_engines::{open_engines, open_roadmap, open_ship, open_signal};

use super::snapshot::NativeRoadmapSnapshot;
use super::tas_config::TasConfig;
use crate::roadmap::RoadmapProgress;

/// The four think-and-ship engines, opened in-process on one shared store.
///
/// Cheap to clone (every engine sits behind an `Arc<Mutex<_>>`); `Send +
/// Sync`. Each typed command takes the one lock it needs and releases it
/// before returning, so no caller can hold an engine across an `await`.
///
/// Persistence is the library's own: each mutation is a locked
/// read-merge-write against `<data_dir>/<family>/sessions/<project_id>.json`,
/// which is exactly what an external `think-and-ship` server does on the
/// same file — two writers merge, neither clobbers.
///
/// The escape hatches (`with_roadmap`, `with_think`, `with_ship`,
/// `with_signal`) exist so the GUI and TUI can reach any engine verb this
/// facade has not wrapped yet, without growing it speculatively.
#[derive(Clone)]
pub struct NativeEngines {
    project_id: String,
    /// The store these engines were opened on — what a reload re-reads.
    config: TasConfig,
    roadmap: Arc<Mutex<RoadmapEngine>>,
    think: Arc<Mutex<ReasoningServer>>,
    ship: Arc<Mutex<ShipEngine>>,
    signal: Arc<Mutex<SignalEngine>>,
}

impl fmt::Debug for NativeEngines {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NativeEngines")
            .field("project_id", &self.project_id)
            .finish_non_exhaustive()
    }
}

/// The kind of check a gate name stands for. An archetype suffix
/// (`test:app-typescript`) does not change the kind, and a custom gate is
/// a lint — the same derivation `magistr_gate` uses for its own records.
fn check_type_for(gate: &str) -> CheckType {
    match gate.split(':').next().unwrap_or(gate) {
        "check" => CheckType::Typecheck,
        "test" => CheckType::Test,
        _ => CheckType::Lint,
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl NativeEngines {
    /// Open the engines for the project at `config.project_root`, resolving
    /// its identity the way the external server would.
    #[must_use]
    pub fn open(config: &TasConfig) -> Self {
        let (project_id, _) = resolve_project_id_with(None, Some(config.project_root()));
        Self::open_for_project(config, project_id)
    }

    /// Open the engines under an explicit project id.
    #[must_use]
    pub fn open_for_project(config: &TasConfig, project_id: String) -> Self {
        // The socket, if any, belongs to the MCP server; this host subscribes
        // to it (see `spawn_broadcast_subscriber`) rather than bind a second.
        let (think, ship, roadmap, signal) = open_engines(config, project_id.clone(), None);

        Self {
            project_id,
            config: config.clone(),
            roadmap: Arc::new(Mutex::new(roadmap)),
            think: Arc::new(Mutex::new(think)),
            ship: Arc::new(Mutex::new(ship)),
            signal: Arc::new(Mutex::new(signal)),
        }
    }

    /// The project every engine is keyed by.
    #[must_use]
    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    /// The store configuration the engines were opened on.
    #[must_use]
    pub fn config(&self) -> &TasConfig {
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

    /// What the store's files look like on disk right now: the newest
    /// modification time across the persisted domains and their total
    /// length. `None` with persistence off, and while nothing has been
    /// written yet.
    ///
    /// A cheap "did anyone else touch this?" for a host that re-reads the
    /// store on a timer (the cockpit's periodic re-seed while no feed is
    /// attached): a reload is not free (and [`Self::reload_ship`] is not
    /// silent), so
    /// a caller compares stamps and reloads only when one moved. The
    /// length is carried beside the time because a filesystem with
    /// one-second mtime granularity would otherwise hide a fast rewrite.
    #[must_use]
    pub fn store_stamp(&self) -> Option<(std::time::SystemTime, u64)> {
        use think_and_ship::infra::{Domain, Persistence};

        let persist = self.config.persistence_config();
        if !persist.enabled {
            return None;
        }
        let mut files: Vec<std::path::PathBuf> = [Domain::Roadmap, Domain::Ship, Domain::Signal]
            .into_iter()
            .map(|domain| Persistence::new(&persist, domain).path_for(&self.project_id))
            .collect();
        files.push(
            think_and_ship::think::persistence::Persistence::for_project(
                &persist,
                &self.project_id,
            )
            .default_store_path(),
        );
        files
            .iter()
            .filter_map(|path| std::fs::metadata(path).ok())
            .filter_map(|meta| Some((meta.modified().ok()?, meta.len())))
            .reduce(|(newest, total), (at, len)| (newest.max(at), total.saturating_add(len)))
    }

    /// The reasoning steps as the store holds them right now, read silently
    /// from the archive another process (the agent's `magistr mcp-serve`) is
    /// writing. The reasoning server is deliberately not re-opened: its load
    /// path can print (legacy-history adoption), which a raw-mode terminal
    /// must never see. Empty when persistence is off.
    #[must_use]
    pub fn think_steps_from_store(&self) -> Vec<ThinkStep> {
        use think_and_ship::think::persistence::{Persistence, read_history};
        let persist = self.config.persistence_config();
        if !persist.enabled {
            return Vec::new();
        }
        let store = Persistence::for_project(&persist, &self.project_id);
        read_history(&store.default_store_path())
            .map(|history| history.steps)
            .unwrap_or_default()
    }

    // ── Escape hatches ─────────────────────────────────────────────────

    /// Run `f` against the roadmap engine under its lock.
    pub fn with_roadmap<R>(&self, f: impl FnOnce(&mut RoadmapEngine) -> R) -> R {
        f(&mut lock(&self.roadmap))
    }

    /// Run `f` against the reasoning engine under its lock.
    pub fn with_think<R>(&self, f: impl FnOnce(&mut ReasoningServer) -> R) -> R {
        f(&mut lock(&self.think))
    }

    /// Run `f` against the execution engine under its lock.
    pub fn with_ship<R>(&self, f: impl FnOnce(&mut ShipEngine) -> R) -> R {
        f(&mut lock(&self.ship))
    }

    /// Run `f` against the signal engine under its lock.
    pub fn with_signal<R>(&self, f: impl FnOnce(&mut SignalEngine) -> R) -> R {
        f(&mut lock(&self.signal))
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

    /// Mark a chunk in progress.
    pub fn roadmap_start_chunk(&self, id: &str) -> Result<Chunk, String> {
        lock(&self.roadmap).start_chunk(id).cloned()
    }

    /// Mark a chunk done, optionally attaching the proof-of-ship ref.
    pub fn roadmap_complete_chunk(
        &self,
        id: &str,
        ship_ref: Option<&str>,
    ) -> Result<Chunk, String> {
        lock(&self.roadmap).complete_chunk(id, ship_ref).cloned()
    }

    /// The markdown (or json) projection `roadmap_export` produces.
    #[must_use]
    pub fn roadmap_export(&self, format: &str) -> String {
        lock(&self.roadmap).export(format)
    }

    /// Add a chunk (the `roadmap_add_chunk` verb). The engine derives `name`
    /// from the id when it is empty and validates content and group.
    // The MCP verb has exactly these fields; a builder here would only
    // re-spell the wire contract.
    #[allow(clippy::too_many_arguments)]
    pub fn roadmap_add_chunk(
        &self,
        id: String,
        title: String,
        name: String,
        status: ChunkStatus,
        priority: u32,
        description: String,
        acceptance: Vec<String>,
        deps: Vec<String>,
        content: Option<StructuredContent>,
        group: Option<String>,
    ) -> Result<Chunk, String> {
        lock(&self.roadmap)
            .add_chunk_with_content(
                id,
                title,
                name,
                status,
                priority,
                description,
                acceptance,
                deps,
                false,
                content,
                group,
            )
            .cloned()
    }

    /// Patch a chunk's descriptive fields (the `roadmap_update_chunk` verb);
    /// `None` leaves a field unchanged.
    // One optional per patchable field, exactly as the MCP verb takes them.
    #[allow(clippy::too_many_arguments)]
    pub fn roadmap_update_chunk(
        &self,
        id: &str,
        title: Option<String>,
        priority: Option<u32>,
        description: Option<String>,
        acceptance: Option<Vec<String>>,
        deps: Option<Vec<String>>,
        content: Option<StructuredContent>,
    ) -> Result<Chunk, String> {
        lock(&self.roadmap)
            .update_chunk(id, title, priority, description, acceptance, deps, content)
            .cloned()
    }

    /// Retract a chunk's blocker (the `roadmap_update_chunk` verb's
    /// `clear_blocked_by`): the dock's \[Clear\] on an awaiting-human card.
    ///
    /// The library errs loudly on a chunk with no blocker rather than
    /// answering a no-op, and this passes that through — a caller who
    /// believes there is a blocker to retract is working from a stale
    /// picture worth interrupting.
    pub fn roadmap_clear_blocked_by(&self, id: &str) -> Result<Chunk, String> {
        lock(&self.roadmap).clear_blocked_by(id).cloned()
    }

    /// Block a chunk on something other than a chunk (the
    /// `roadmap_update_chunk` verb's `blocked_by`): validated the way the
    /// MCP verb validates it, then stored. An `AwaitingHuman` blocker is
    /// what the cockpit's dock raises a card for.
    ///
    /// # Errors
    /// The library's reason — an unknown id, or a reason the validation
    /// refuses.
    pub fn roadmap_set_blocked_by(
        &self,
        id: &str,
        kind: BlockerKind,
        reason: String,
        evidence: Option<String>,
    ) -> Result<Chunk, String> {
        let mut roadmap = lock(&self.roadmap);
        let blocked = roadmap.validate_blocked_by(kind, reason, evidence)?;
        roadmap.set_blocked_by(id, blocked).cloned()
    }

    /// Transition a chunk's status, validated against the lifecycle table.
    pub fn roadmap_set_status(&self, id: &str, to: ChunkStatus) -> Result<Chunk, String> {
        lock(&self.roadmap).set_status(id, to).cloned()
    }

    /// Mark a chunk obsoleted with a reason.
    pub fn roadmap_obsolete_chunk(&self, id: &str, reason: String) -> Result<Chunk, String> {
        lock(&self.roadmap).obsolete_chunk(id, reason).cloned()
    }

    /// Record a re-prioritization PROPOSAL (the priority itself is unchanged).
    pub fn roadmap_reprioritize(
        &self,
        id: &str,
        suggested_priority: u32,
        reason: String,
    ) -> Result<Chunk, String> {
        lock(&self.roadmap)
            .propose_reprioritize(id, suggested_priority, reason)
            .cloned()
    }

    /// Attach a `think:`/`task:`/`action:`/`check:`/`chunk:` cross-ref.
    pub fn roadmap_link(&self, id: &str, cross_ref: &str) -> Result<Chunk, String> {
        lock(&self.roadmap).link_chunk(id, cross_ref).cloned()
    }

    /// Move a chunk into a workstream (`None` ungroups it).
    pub fn roadmap_set_group(&self, id: &str, group: Option<String>) -> Result<Chunk, String> {
        lock(&self.roadmap).set_group(id, group).cloned()
    }

    /// Resolve an open contested-title proposal: accept adopts the
    /// suggested title into the plan, reject clears the proposal so the
    /// plan's title flows again.
    pub fn roadmap_resolve_title_proposal(&self, id: &str, accept: bool) -> Result<Chunk, String> {
        lock(&self.roadmap)
            .resolve_title_proposal(id, accept)
            .cloned()
    }

    /// Resolve an open status proposal: accept takes the suggested
    /// transition, reject re-asserts the current status. The library has
    /// no reject verb for this kind — any explicit transition disposes
    /// the proposal, and identity transitions are legal, so "reject" is
    /// the transition to where the chunk already stands.
    pub fn roadmap_resolve_status_proposal(&self, id: &str, accept: bool) -> Result<Chunk, String> {
        let mut roadmap = lock(&self.roadmap);
        let chunk = roadmap
            .roadmap()
            .chunks
            .iter()
            .find(|c| c.id == id)
            .ok_or_else(|| format!("no chunk '{id}'"))?;
        let Some(proposal) = chunk.status_proposal.as_ref() else {
            return Err(format!("chunk '{id}' has no open status proposal"));
        };
        let to = if accept {
            proposal.suggested_status
        } else {
            chunk.status
        };
        roadmap.set_status(id, to).cloned()
    }

    /// Resolve an open re-prioritization proposal: accept takes the
    /// suggested priority, reject keeps the current one. The library
    /// disposes this kind only on a *differing* priority edit, so when
    /// the resolved value equals the chunk's priority the edit passes
    /// through a neighbouring value and back — two frames, same final
    /// state, proposal disposed.
    pub fn roadmap_resolve_reprioritize(&self, id: &str, accept: bool) -> Result<Chunk, String> {
        let mut roadmap = lock(&self.roadmap);
        let chunk = roadmap
            .roadmap()
            .chunks
            .iter()
            .find(|c| c.id == id)
            .ok_or_else(|| format!("no chunk '{id}'"))?;
        let Some(proposal) = chunk.reprioritize.as_ref() else {
            return Err(format!("chunk '{id}' has no open reprioritize proposal"));
        };
        let current = chunk.priority;
        let target = if accept {
            proposal.suggested_priority
        } else {
            current
        };
        if target != current {
            return roadmap
                .update_chunk(id, None, Some(target), None, None, None, None)
                .cloned();
        }
        let detour = if target == u32::MAX {
            target - 1
        } else {
            target + 1
        };
        roadmap.update_chunk(id, None, Some(detour), None, None, None, None)?;
        roadmap
            .update_chunk(id, None, Some(target), None, None, None, None)
            .cloned()
    }

    // ── Think ──────────────────────────────────────────────────────────

    /// Record one reasoning step (the `think_record_step` verb).
    pub fn think_record_step(&self, step: ThinkStep) -> ProcessResult {
        lock(&self.think).process_step(step)
    }

    /// Pin or unpin a step.
    pub fn think_pin_step(&self, step_number: u32, pinned: bool) -> Result<bool, String> {
        lock(&self.think).pin_step(step_number, pinned)
    }

    /// The trace-wide metacognitive checkpoint (the
    /// `think_trace_checkpoint` verb's report): open hypotheses, stale
    /// branches, confidence drift, dependencies on revised steps.
    #[must_use]
    pub fn think_checkpoint(&self) -> serde_json::Value {
        lock(&self.think).checkpoint_snapshot()
    }

    // ── Ship ───────────────────────────────────────────────────────────

    /// Define the objective; returns how many stale tasks a fresh cycle cleared.
    pub fn ship_set_objective(
        &self,
        description: String,
        acceptance_criteria: Vec<String>,
        constraints: Vec<String>,
        scope: String,
    ) -> usize {
        lock(&self.ship).set_objective(description, acceptance_criteria, constraints, scope)
    }

    /// Add a task to the plan.
    pub fn ship_add_task(
        &self,
        id: String,
        title: String,
        task_type: TaskType,
        estimate: Option<String>,
    ) {
        lock(&self.ship).add_task(id, title, task_type, estimate, None);
    }

    /// Begin a task.
    pub fn ship_start_task(&self, task_id: &str) -> Result<Task, String> {
        lock(&self.ship).start_task(task_id).cloned()
    }

    /// Log an action within a task (the active one when `task_id` is `None`).
    pub fn ship_record_action(
        &self,
        task_id: Option<&str>,
        action_type: ActionType,
        description: String,
        files_touched: Vec<String>,
        think_step: Option<u32>,
    ) -> Result<Action, String> {
        lock(&self.ship)
            .record_action(
                task_id,
                action_type,
                description,
                files_touched,
                Vec::new(),
                String::new(),
                think_step,
            )
            .cloned()
    }

    /// Record a quality gate that a command actually ran: `passed` is the
    /// exit code, and the check is `verified` — the same record `ship_check`
    /// writes when it executes the command itself. This is how the loop
    /// driver's between-iteration gates reach the trace; they used to write
    /// nothing at all, while the agent's own `magistr_gate` wrote verified
    /// records beside them.
    ///
    /// The check's kind is derived from `gate`, which may carry an
    /// archetype suffix (`test:app-typescript`): the part before the `:`
    /// decides, and anything that is not `check` or `test` is a lint —
    /// the same fallback `magistr_gate`'s own mapping uses.
    ///
    /// `report` is an optional `(format, path)` a runner wrote, parsed into
    /// the record additively: the path is used verbatim, so pass an absolute
    /// one, and a file that is missing or unreadable comes back "not
    /// parsed" rather than failing the check. The exit code decides
    /// `passed`, never the report.
    ///
    /// # Errors
    /// The engine's own reason — typically that no task is active to
    /// record the check against. A caller must say so rather than record
    /// nothing silently.
    #[allow(clippy::too_many_arguments)]
    pub fn ship_record_gate(
        &self,
        task_id: Option<&str>,
        gate: &str,
        command: String,
        exit_code: i32,
        details: String,
        required: bool,
        report: Option<(&str, &str)>,
    ) -> Result<Check, String> {
        let (record, results) = match report {
            Some((format, path)) => {
                let (record, results) = think_and_ship::ship::report::read_report(format, path);
                (Some(record), results)
            }
            None => (None, None),
        };
        lock(&self.ship)
            .record_check_full(
                task_id,
                check_type_for(gate),
                gate.to_owned(),
                exit_code == 0,
                details,
                required,
                true,
                Some(command),
                Some(exit_code),
                record,
                results,
            )
            .cloned()
    }

    /// Close a task.
    pub fn ship_complete_task(
        &self,
        task_id: &str,
        artifacts: Vec<Artifact>,
    ) -> Result<Task, String> {
        lock(&self.ship).complete_task(task_id, artifacts).cloned()
    }

    /// Finalize the objective (the `ship_finalize` verb); returns the report.
    pub fn ship_finalize(
        &self,
        artifacts: Vec<Artifact>,
        summary: Option<String>,
    ) -> serde_json::Value {
        lock(&self.ship).ship(artifacts, summary)
    }

    /// The full execution-state snapshot.
    #[must_use]
    pub fn ship_status(&self) -> serde_json::Value {
        lock(&self.ship).status()
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

    /// Capture a stakeholder signal.
    pub fn signal_capture(&self, kind: SignalKind, from: String, body: String) -> Signal {
        lock(&self.signal).capture(kind, from, body).clone()
    }

    /// Signal status counts.
    #[must_use]
    pub fn signal_status(&self) -> serde_json::Value {
        lock(&self.signal).status()
    }

    /// Every signal in the store, capture order.
    #[must_use]
    pub fn signal_list(&self) -> Vec<Signal> {
        lock(&self.signal).signals().signals.clone()
    }

    /// Promote a researched or surfaced signal to the roadmap.
    pub fn signal_promote(&self, id: &str) -> Result<Signal, String> {
        lock(&self.signal).promote(id).cloned()
    }

    /// Dismiss a signal (terminal).
    pub fn signal_ignore(&self, id: &str) -> Result<Signal, String> {
        lock(&self.signal).ignore(id).cloned()
    }

    /// Suppress a signal from surfacing for `minutes`; status unchanged.
    pub fn signal_snooze(&self, id: &str, minutes: i64) -> Result<Signal, String> {
        lock(&self.signal).snooze(id, minutes).cloned()
    }
}

#[cfg(test)]
#[path = "native_engines.tests.rs"]
mod tests;
