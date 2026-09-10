use std::fmt;
use std::ops::Deref;
use std::sync::Arc;

use think_and_ship::content::StructuredContent;
use think_and_ship::roadmap::domain::{BlockerKind, Chunk, ChunkStatus};
use think_and_ship::ship::domain::action::{Action, ActionType};
use think_and_ship::ship::domain::artifact::Artifact;
use think_and_ship::ship::domain::check::{Check, CheckType};
use think_and_ship::ship::domain::task::{Task, TaskType};
use think_and_ship::signal::domain::{Signal, SignalKind};
use think_and_ship::think::domain::ThinkStep;
use think_and_ship::think::engine::ProcessResult;

use super::native_reader::{NativeReader, lock};
use super::project_authority::ProjectAuthority;

/// The write capability over the four think-and-ship engines, held only by
/// an admitted [`ProjectAuthority`] — [`ProjectAuthority::open`] is the one
/// constructor. Derefs to the [`NativeReader`] it was opened with, so every
/// query is available on it too.
///
/// Cheap to clone (the engines sit behind `Arc<Mutex<_>>`, the admission
/// behind an `Arc`); `Send + Sync`. Every clone carries the lease, so the
/// role's lock is held for as long as any writer lives and released with
/// the last one. Each typed command takes the one engine lock it needs and
/// releases it before returning — the short critical section — so no caller
/// can hold an engine across an `await` or run arbitrary code under its lock.
///
/// Persistence is the library's own: each mutation is a locked
/// read-merge-write against `<data_dir>/<family>/sessions/<project_id>.json`,
/// which is exactly what an external `think-and-ship` server does on the
/// same file — two writers merge, neither clobbers.
#[derive(Clone)]
pub struct NativeEngines {
    reader: NativeReader,
    authority: Arc<ProjectAuthority>,
}

impl fmt::Debug for NativeEngines {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NativeEngines")
            .field("project_id", &self.reader.project_id())
            .field("role", &self.authority.role())
            .finish_non_exhaustive()
    }
}

impl Deref for NativeEngines {
    type Target = NativeReader;

    fn deref(&self) -> &NativeReader {
        &self.reader
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

impl NativeEngines {
    pub(crate) fn admitted(reader: NativeReader, authority: Arc<ProjectAuthority>) -> Self {
        Self { reader, authority }
    }

    /// The read capability over the same engines, without the lease: what a
    /// re-seed timer or a progress view holds so that dropping the last
    /// writer releases the role's lock regardless of how many observers
    /// remain.
    #[must_use]
    pub fn reader(&self) -> NativeReader {
        self.reader.clone()
    }

    /// The admission this writer holds.
    #[must_use]
    pub fn authority(&self) -> &ProjectAuthority {
        &self.authority
    }

    // ── Roadmap ────────────────────────────────────────────────────────

    /// Mark a chunk in progress.
    pub fn roadmap_start_chunk(&self, id: &str) -> Result<Chunk, String> {
        lock(self.roadmap_engine()).start_chunk(id).cloned()
    }

    /// Mark a chunk done, optionally attaching the proof-of-ship ref.
    pub fn roadmap_complete_chunk(
        &self,
        id: &str,
        ship_ref: Option<&str>,
    ) -> Result<Chunk, String> {
        lock(self.roadmap_engine())
            .complete_chunk(id, ship_ref)
            .cloned()
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
        lock(self.roadmap_engine())
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
        lock(self.roadmap_engine())
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
        lock(self.roadmap_engine()).clear_blocked_by(id).cloned()
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
        let mut roadmap = lock(self.roadmap_engine());
        let blocked = roadmap.validate_blocked_by(kind, reason, evidence)?;
        roadmap.set_blocked_by(id, blocked).cloned()
    }

    /// Transition a chunk's status, validated against the lifecycle table.
    pub fn roadmap_set_status(&self, id: &str, to: ChunkStatus) -> Result<Chunk, String> {
        lock(self.roadmap_engine()).set_status(id, to).cloned()
    }

    /// Mark a chunk obsoleted with a reason.
    pub fn roadmap_obsolete_chunk(&self, id: &str, reason: String) -> Result<Chunk, String> {
        lock(self.roadmap_engine())
            .obsolete_chunk(id, reason)
            .cloned()
    }

    /// Record a re-prioritization PROPOSAL (the priority itself is unchanged).
    pub fn roadmap_reprioritize(
        &self,
        id: &str,
        suggested_priority: u32,
        reason: String,
    ) -> Result<Chunk, String> {
        lock(self.roadmap_engine())
            .propose_reprioritize(id, suggested_priority, reason)
            .cloned()
    }

    /// Record a contested-title PROPOSAL from `source` (a tracker mirror);
    /// the plan's title is unchanged until it is resolved.
    pub fn roadmap_propose_title(
        &self,
        id: &str,
        suggested_title: String,
        reason: String,
        source: String,
    ) -> Result<Chunk, String> {
        lock(self.roadmap_engine())
            .propose_title(id, suggested_title, reason, source)
            .cloned()
    }

    /// Record a status PROPOSAL from `source`; the chunk's status is
    /// unchanged until it is resolved.
    pub fn roadmap_propose_status(
        &self,
        id: &str,
        suggested_status: ChunkStatus,
        reason: String,
        source: String,
    ) -> Result<Chunk, String> {
        lock(self.roadmap_engine())
            .propose_status(id, suggested_status, reason, source)
            .cloned()
    }

    /// Attach a `think:`/`task:`/`action:`/`check:`/`chunk:` cross-ref.
    pub fn roadmap_link(&self, id: &str, cross_ref: &str) -> Result<Chunk, String> {
        lock(self.roadmap_engine())
            .link_chunk(id, cross_ref)
            .cloned()
    }

    /// Move a chunk into a workstream (`None` ungroups it).
    pub fn roadmap_set_group(&self, id: &str, group: Option<String>) -> Result<Chunk, String> {
        lock(self.roadmap_engine()).set_group(id, group).cloned()
    }

    /// Resolve an open contested-title proposal: accept adopts the
    /// suggested title into the plan, reject clears the proposal so the
    /// plan's title flows again.
    pub fn roadmap_resolve_title_proposal(&self, id: &str, accept: bool) -> Result<Chunk, String> {
        lock(self.roadmap_engine())
            .resolve_title_proposal(id, accept)
            .cloned()
    }

    /// Resolve an open status proposal: accept takes the suggested
    /// transition, reject re-asserts the current status. The library has
    /// no reject verb for this kind — any explicit transition disposes
    /// the proposal, and identity transitions are legal, so "reject" is
    /// the transition to where the chunk already stands.
    pub fn roadmap_resolve_status_proposal(&self, id: &str, accept: bool) -> Result<Chunk, String> {
        let mut roadmap = lock(self.roadmap_engine());
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
        let mut roadmap = lock(self.roadmap_engine());
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
        lock(self.think_engine()).process_step(step)
    }

    /// Pin or unpin a step.
    pub fn think_pin_step(&self, step_number: u32, pinned: bool) -> Result<bool, String> {
        lock(self.think_engine()).pin_step(step_number, pinned)
    }

    /// The trace-wide metacognitive checkpoint (the
    /// `think_trace_checkpoint` verb's report): open hypotheses, stale
    /// branches, confidence drift, dependencies on revised steps.
    #[must_use]
    pub fn think_checkpoint(&self) -> serde_json::Value {
        lock(self.think_engine()).checkpoint_snapshot()
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
        lock(self.ship_engine()).set_objective(description, acceptance_criteria, constraints, scope)
    }

    /// Add a task to the plan.
    pub fn ship_add_task(
        &self,
        id: String,
        title: String,
        task_type: TaskType,
        estimate: Option<String>,
    ) {
        lock(self.ship_engine()).add_task(id, title, task_type, estimate, None);
    }

    /// Begin a task.
    pub fn ship_start_task(&self, task_id: &str) -> Result<Task, String> {
        lock(self.ship_engine()).start_task(task_id).cloned()
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
        lock(self.ship_engine())
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
        lock(self.ship_engine())
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
        lock(self.ship_engine())
            .complete_task(task_id, artifacts)
            .cloned()
    }

    /// Finalize the objective (the `ship_finalize` verb); returns the report.
    pub fn ship_finalize(
        &self,
        artifacts: Vec<Artifact>,
        summary: Option<String>,
    ) -> serde_json::Value {
        lock(self.ship_engine()).ship(artifacts, summary)
    }

    // ── Signal ─────────────────────────────────────────────────────────

    /// Capture a stakeholder signal.
    pub fn signal_capture(&self, kind: SignalKind, from: String, body: String) -> Signal {
        lock(self.signal_engine()).capture(kind, from, body).clone()
    }

    /// Record research on a signal (the `signal_research` verb): a summary,
    /// a confidence clamped to `[0, 1]`, sources, and the reasoning step
    /// that motivated it. Moves a new signal to `Researched`.
    pub fn signal_research(
        &self,
        id: &str,
        summary: String,
        confidence: f64,
        sources: Vec<String>,
        think_step: Option<u32>,
    ) -> Result<Signal, String> {
        lock(self.signal_engine())
            .research(id, summary, confidence, sources, think_step)
            .cloned()
    }

    /// Promote a researched or surfaced signal to the roadmap.
    pub fn signal_promote(&self, id: &str) -> Result<Signal, String> {
        lock(self.signal_engine()).promote(id).cloned()
    }

    /// Dismiss a signal (terminal).
    pub fn signal_ignore(&self, id: &str) -> Result<Signal, String> {
        lock(self.signal_engine()).ignore(id).cloned()
    }

    /// Suppress a signal from surfacing for `minutes`; status unchanged.
    pub fn signal_snooze(&self, id: &str, minutes: i64) -> Result<Signal, String> {
        lock(self.signal_engine()).snooze(id, minutes).cloned()
    }
}

#[cfg(test)]
#[path = "native_engines.tests.rs"]
mod tests;
