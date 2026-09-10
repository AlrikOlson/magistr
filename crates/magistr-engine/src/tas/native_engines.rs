use std::fmt;
use std::ops::Deref;
use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use think_and_ship::content::StructuredContent;
use think_and_ship::roadmap::domain::{BlockerKind, Chunk, ChunkStatus};
use think_and_ship::roadmap::mcp::handlers as roadmap;
use think_and_ship::ship::domain::action::{Action, ActionType};
use think_and_ship::ship::domain::artifact::Artifact;
use think_and_ship::ship::domain::check::{Check, CheckType};
use think_and_ship::ship::domain::task::{Task, TaskType};
use think_and_ship::ship::mcp::handlers as ship;
use think_and_ship::signal::domain::{Signal, SignalKind};
use think_and_ship::signal::mcp::handlers as signal;
use think_and_ship::think::domain::ThinkStep;
use think_and_ship::think::engine::{ProcessErr, ProcessOk, ProcessResult};
use think_and_ship::think::mcp::args as think;

use super::family_services::FamilyServices;
use super::handler_call::settle;
use super::native_reader::{NativeReader, lock};
use super::project_authority::ProjectAuthority;

/// The write capability over the four think-and-ship engines, held only by
/// an admitted [`ProjectAuthority`] — [`ProjectAuthority::open`] is the one
/// constructor. Derefs to the [`NativeReader`] it was opened with, so every
/// query is available on it too.
///
/// Every verb here is carried out by the library's own tool handler — the
/// method behind the MCP tool of the same name, called directly on the
/// family services the authority composed — and its structured answer is
/// read back into the typed value. So the agent's `roadmap_start_chunk`
/// over MCP, the cockpit's \[Start\], a second cockpit's command over the
/// owner endpoint and the loop driver's completion all run **one**
/// implementation: the same validation, the same reciprocal cross-family
/// behavior (`signal_promote` creates and links the backlog chunk), the
/// same short critical section on the same engine lock, the same
/// persistence. There is no parallel implementation here to keep in step.
///
/// Two verbs are the exception, each a direct call of exactly the engine
/// method the wire's handler calls itself: [`Self::ship_record_gate`]
/// (`ship_check`'s handler runs the command before recording it; Magistr's
/// runner already ran the gate, so only the recording half applies) and
/// [`Self::roadmap_propose_title`] / [`Self::roadmap_propose_status`]
/// (tracker-mirror verbs with no MCP tool; the library's own mirror calls
/// the engine the same way). The inventory gate pins that no other engine
/// call exists outside this file and the reader.
///
/// Cheap to clone (the engines sit behind `Arc<Mutex<_>>`, the admission
/// behind an `Arc`); `Send + Sync`. Every clone carries the lease, so the
/// role's lock is held for as long as any writer lives and released with
/// the last one.
///
/// Persistence is the library's own: each mutation is a locked
/// read-merge-write against `<data_dir>/<family>/sessions/<project_id>.json`,
/// which is exactly what an external `think-and-ship` server does on the
/// same file — two writers merge, neither clobbers.
#[derive(Clone)]
pub struct NativeEngines {
    reader: NativeReader,
    authority: Arc<ProjectAuthority>,
    services: FamilyServices,
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

/// A wire enum's spelling — what the handler's string argument takes.
fn wire<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(Value::String(spelling)) => spelling,
        Ok(other) => other.to_string(),
        Err(_) => String::new(),
    }
}

/// A handler's structured answer as the typed value it serialized.
fn decode<T: DeserializeOwned>(value: Value) -> Result<T, String> {
    serde_json::from_value(value).map_err(|error| format!("the handler's answer: {error}"))
}

/// One field of a handler's structured answer.
fn field(mut value: Value, key: &str) -> Result<Value, String> {
    match value.get_mut(key).map(Value::take) {
        Some(inner) => Ok(inner),
        None => Err(format!("the handler's answer carries no `{key}`: {value}")),
    }
}

/// A `roadmap_update_chunk` call that patches nothing unless a field is set.
fn patch(id: &str) -> roadmap::UpdateChunkArgs {
    roadmap::UpdateChunkArgs {
        id: id.to_owned(),
        title: None,
        name: None,
        priority: None,
        description: None,
        acceptance: None,
        deps: None,
        content: None,
        resolve_title_proposal: None,
        blocked_by: None,
        clear_blocked_by: false,
    }
}

impl NativeEngines {
    pub(crate) fn admitted(
        reader: NativeReader,
        authority: Arc<ProjectAuthority>,
        services: FamilyServices,
    ) -> Self {
        Self {
            reader,
            authority,
            services,
        }
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

    /// The admission, shared — what an owner endpoint keys itself by and
    /// what a transport keeps alive beside its wire.
    #[must_use]
    pub fn shared_authority(&self) -> Arc<ProjectAuthority> {
        Arc::clone(&self.authority)
    }

    /// The family services every verb runs through — what a transport
    /// serves on the wire.
    pub(crate) fn services(&self) -> &FamilyServices {
        &self.services
    }

    // ── Roadmap ────────────────────────────────────────────────────────

    /// Mark a chunk in progress (the `roadmap_start_chunk` verb).
    pub fn roadmap_start_chunk(&self, id: &str) -> Result<Chunk, String> {
        let args = roadmap::StartChunkArgs { id: id.to_owned() };
        let answer = settle(self.services.roadmap.roadmap_start_chunk(Parameters(args)))?;
        decode(field(answer, "chunk")?)
    }

    /// Mark a chunk done, optionally attaching the proof-of-ship ref (the
    /// `roadmap_complete_chunk` verb).
    pub fn roadmap_complete_chunk(
        &self,
        id: &str,
        ship_ref: Option<&str>,
    ) -> Result<Chunk, String> {
        let args = roadmap::CompleteChunkArgs {
            id: id.to_owned(),
            ship_ref: ship_ref.map(str::to_owned),
        };
        decode(settle(
            self.services
                .roadmap
                .roadmap_complete_chunk(Parameters(args)),
        )?)
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
        let args = roadmap::AddChunkArgs {
            id,
            title,
            name,
            status: Some(wire(&status)),
            priority,
            description,
            acceptance,
            deps,
            shared: false,
            group,
            content: content.map(|content| serde_json::to_value(content).unwrap_or(Value::Null)),
        };
        decode(settle(
            self.services.roadmap.roadmap_add_chunk(Parameters(args)),
        )?)
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
        let args = roadmap::UpdateChunkArgs {
            title,
            priority,
            description,
            acceptance,
            deps,
            content: content.map(|content| serde_json::to_value(content).unwrap_or(Value::Null)),
            ..patch(id)
        };
        self.update_chunk(args)
    }

    fn update_chunk(&self, args: roadmap::UpdateChunkArgs) -> Result<Chunk, String> {
        decode(settle(
            self.services.roadmap.roadmap_update_chunk(Parameters(args)),
        )?)
    }

    /// Retract a chunk's blocker (the `roadmap_update_chunk` verb's
    /// `clear_blocked_by`): the dock's \[Clear\] on an awaiting-human card.
    ///
    /// The library errs loudly on a chunk with no blocker rather than
    /// answering a no-op, and this passes that through — a caller who
    /// believes there is a blocker to retract is working from a stale
    /// picture worth interrupting.
    pub fn roadmap_clear_blocked_by(&self, id: &str) -> Result<Chunk, String> {
        self.update_chunk(roadmap::UpdateChunkArgs {
            clear_blocked_by: true,
            ..patch(id)
        })
    }

    /// Block a chunk on something other than a chunk (the
    /// `roadmap_update_chunk` verb's `blocked_by`). An `AwaitingHuman`
    /// blocker is what the cockpit's dock raises a card for.
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
        self.update_chunk(roadmap::UpdateChunkArgs {
            blocked_by: Some(roadmap::BlockedByArgs {
                kind: kind.as_wire().to_owned(),
                reason,
                evidence,
            }),
            ..patch(id)
        })
    }

    /// Transition a chunk's status, validated against the lifecycle table
    /// (the `roadmap_set_status` verb).
    pub fn roadmap_set_status(&self, id: &str, to: ChunkStatus) -> Result<Chunk, String> {
        let args = roadmap::SetStatusArgs {
            id: id.to_owned(),
            status: wire(&to),
        };
        decode(settle(
            self.services.roadmap.roadmap_set_status(Parameters(args)),
        )?)
    }

    /// Mark a chunk obsoleted with a reason (the `roadmap_obsolete_chunk` verb).
    pub fn roadmap_obsolete_chunk(&self, id: &str, reason: String) -> Result<Chunk, String> {
        let args = roadmap::ObsoleteArgs {
            id: id.to_owned(),
            reason,
        };
        decode(settle(
            self.services
                .roadmap
                .roadmap_obsolete_chunk(Parameters(args)),
        )?)
    }

    /// Record a re-prioritization PROPOSAL (the `roadmap_reprioritize`
    /// verb); the priority itself is unchanged.
    pub fn roadmap_reprioritize(
        &self,
        id: &str,
        suggested_priority: u32,
        reason: String,
    ) -> Result<Chunk, String> {
        let args = roadmap::ReprioritizeArgs {
            id: id.to_owned(),
            suggested_priority,
            reason,
        };
        decode(settle(
            self.services.roadmap.roadmap_reprioritize(Parameters(args)),
        )?)
    }

    /// Record a contested-title PROPOSAL from `source` (a tracker mirror);
    /// the plan's title is unchanged until it is resolved.
    ///
    /// No MCP tool carries this verb — it is the tracker mirror's — so it
    /// is the engine method itself, as the library's own mirror calls it.
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
    /// unchanged until it is resolved. The engine method itself, for the
    /// reason [`Self::roadmap_propose_title`] gives.
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

    /// Attach a `think:`/`task:`/`action:`/`check:`/`chunk:` cross-ref (the
    /// `roadmap_link` verb).
    pub fn roadmap_link(&self, id: &str, cross_ref: &str) -> Result<Chunk, String> {
        let args = roadmap::LinkArgs {
            id: id.to_owned(),
            cross_ref: cross_ref.to_owned(),
        };
        decode(settle(
            self.services.roadmap.roadmap_link(Parameters(args)),
        )?)
    }

    /// Move a chunk into a workstream (the `roadmap_set_group` verb; `None`
    /// ungroups it).
    pub fn roadmap_set_group(&self, id: &str, group: Option<String>) -> Result<Chunk, String> {
        let args = roadmap::SetGroupArgs {
            id: id.to_owned(),
            group,
        };
        decode(settle(
            self.services.roadmap.roadmap_set_group(Parameters(args)),
        )?)
    }

    /// Resolve an open contested-title proposal (the `roadmap_update_chunk`
    /// verb's `resolve_title_proposal`): accept adopts the suggested title
    /// into the plan, reject clears the proposal so the plan's title flows
    /// again.
    pub fn roadmap_resolve_title_proposal(&self, id: &str, accept: bool) -> Result<Chunk, String> {
        self.update_chunk(roadmap::UpdateChunkArgs {
            resolve_title_proposal: Some(if accept { "accept" } else { "reject" }.to_owned()),
            ..patch(id)
        })
    }

    /// Resolve an open status proposal: accept takes the suggested
    /// transition, reject re-asserts the current status. The library has
    /// no reject verb for this kind — any explicit transition disposes
    /// the proposal, and identity transitions are legal, so "reject" is
    /// the `roadmap_set_status` to where the chunk already stands.
    pub fn roadmap_resolve_status_proposal(&self, id: &str, accept: bool) -> Result<Chunk, String> {
        let chunk = self
            .roadmap_chunk(id)
            .ok_or_else(|| format!("no chunk '{id}'"))?;
        let Some(proposal) = chunk.status_proposal.as_ref() else {
            return Err(format!("chunk '{id}' has no open status proposal"));
        };
        let to = if accept {
            proposal.suggested_status
        } else {
            chunk.status
        };
        self.roadmap_set_status(id, to)
    }

    /// Resolve an open re-prioritization proposal: accept takes the
    /// suggested priority, reject keeps the current one. The library
    /// disposes this kind only on a *differing* priority edit, so when
    /// the resolved value equals the chunk's priority the edit passes
    /// through a neighbouring value and back — two `roadmap_update_chunk`
    /// calls, same final state, proposal disposed.
    pub fn roadmap_resolve_reprioritize(&self, id: &str, accept: bool) -> Result<Chunk, String> {
        let chunk = self
            .roadmap_chunk(id)
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
        let priority = |priority: u32| roadmap::UpdateChunkArgs {
            priority: Some(priority),
            ..patch(id)
        };
        if target != current {
            return self.update_chunk(priority(target));
        }
        let detour = if target == u32::MAX {
            target - 1
        } else {
            target + 1
        };
        self.update_chunk(priority(detour))?;
        self.update_chunk(priority(target))
    }

    // ── Think ──────────────────────────────────────────────────────────

    /// Record one reasoning step (the `think_record_step` verb). The
    /// answer's text is the handler's structured document, the same one
    /// the wire carries.
    pub fn think_record_step(&self, step: ThinkStep) -> ProcessResult {
        match settle(self.services.think.think_record_step(Parameters(step))) {
            Ok(Value::String(text)) => Ok(ProcessOk { text }),
            Ok(document) => Ok(ProcessOk {
                text: document.to_string(),
            }),
            Err(text) => Err(ProcessErr { text }),
        }
    }

    /// Pin or unpin a step (the `think_pin_step` verb); answers whether it
    /// was pinned before.
    pub fn think_pin_step(&self, step_number: u32, pinned: bool) -> Result<bool, String> {
        let args = think::PinArgs {
            step_number,
            pinned: Some(pinned),
        };
        let answer = settle(self.services.think.think_pin_step(Parameters(args)))?;
        answer["was_pinned"]
            .as_bool()
            .ok_or_else(|| format!("the handler's answer carries no `was_pinned`: {answer}"))
    }

    /// The trace-wide metacognitive checkpoint (the
    /// `think_trace_checkpoint` verb's report): open hypotheses, stale
    /// branches, confidence drift, dependencies on revised steps.
    pub fn think_checkpoint(&self) -> Result<Value, String> {
        settle(
            self.services
                .think
                .think_trace_checkpoint(Parameters(think::NoArgs {})),
        )
    }

    // ── Ship ───────────────────────────────────────────────────────────

    /// Define the objective (the `ship_set_objective` verb); answers how
    /// many stale tasks a fresh cycle cleared.
    pub fn ship_set_objective(
        &self,
        description: String,
        acceptance_criteria: Vec<String>,
        constraints: Vec<String>,
        scope: String,
    ) -> Result<usize, String> {
        let args = ship::SetObjectiveArgs {
            description,
            acceptance_criteria,
            constraints,
            scope,
        };
        let answer = settle(self.services.ship.ship_set_objective(Parameters(args)))?;
        Ok(answer["cleared_stale_tasks"]
            .as_u64()
            .and_then(|cleared| usize::try_from(cleared).ok())
            .unwrap_or(0))
    }

    /// Add a task to the plan (the `ship_plan` verb's `add`).
    pub fn ship_add_task(
        &self,
        id: String,
        title: String,
        task_type: TaskType,
        estimate: Option<String>,
    ) -> Result<(), String> {
        let args = ship::PlanArgs {
            action: ship::PlanAction::Add,
            task_id: id,
            title: Some(title),
            task_type: Some(task_type),
            estimate,
            after: None,
            think_branch: None,
        };
        settle(self.services.ship.ship_plan(Parameters(args))).map(|_| ())
    }

    /// Begin a task (the `ship_start` verb).
    pub fn ship_start_task(&self, task_id: &str) -> Result<Task, String> {
        let args = ship::StartArgs {
            task_id: task_id.to_owned(),
        };
        decode(settle(self.services.ship.ship_start(Parameters(args)))?)
    }

    /// Log an action within a task (the `ship_record` verb; the active
    /// task when `task_id` is `None`).
    pub fn ship_record_action(
        &self,
        task_id: Option<&str>,
        action_type: ActionType,
        description: String,
        files_touched: Vec<String>,
        tools_used: Vec<String>,
        think_step: Option<u32>,
    ) -> Result<Action, String> {
        let args = ship::RecordArgs {
            task_id: task_id.map(str::to_owned),
            action_type,
            description,
            files_touched,
            tools_used,
            result: String::new(),
            think_step,
        };
        decode(settle(self.services.ship.ship_record(Parameters(args)))?)
    }

    /// Record a quality gate that a command actually ran: `passed` is the
    /// exit code, and the check is `verified` — the same record `ship_check`
    /// writes when it executes the command itself. This is how the loop
    /// driver's between-iteration gates and `magistr_gate`'s runs reach
    /// the trace.
    ///
    /// The one verb the wire's handler cannot carry out for a caller:
    /// `ship_check` runs the command *itself* before recording, and the
    /// gate here has already run on Magistr's runner (on a pty, streamed to
    /// the feed). So this is the recording half alone — `record_check_full`,
    /// the very engine method `ship_check`'s handler calls once its command
    /// has returned, with the same arguments that handler passes.
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

    /// Close a task (the `ship_complete` verb).
    pub fn ship_complete_task(
        &self,
        task_id: &str,
        artifacts: Vec<Artifact>,
    ) -> Result<Task, String> {
        let args = ship::CompleteArgs {
            task_id: task_id.to_owned(),
            artifacts: artifacts
                .into_iter()
                .map(|artifact| ship::ArtifactInput {
                    artifact_type: artifact.artifact_type,
                    reference: artifact.reference,
                    description: artifact.description,
                })
                .collect(),
        };
        decode(settle(self.services.ship.ship_complete(Parameters(args)))?)
    }

    /// Finalize the objective (the `ship_finalize` verb); answers the report.
    pub fn ship_finalize(
        &self,
        artifacts: Vec<Artifact>,
        summary: Option<String>,
    ) -> Result<Value, String> {
        let args = ship::ShipArgs {
            artifacts: artifacts
                .into_iter()
                .map(|artifact| ship::ArtifactInput {
                    artifact_type: artifact.artifact_type,
                    reference: artifact.reference,
                    description: artifact.description,
                })
                .collect(),
            summary,
        };
        settle(self.services.ship.ship_finalize(Parameters(args)))
    }

    // ── Signal ─────────────────────────────────────────────────────────

    /// Capture a stakeholder signal (the `signal_capture` verb). An empty
    /// body is refused, as the verb refuses it.
    pub fn signal_capture(
        &self,
        kind: SignalKind,
        from: String,
        body: String,
    ) -> Result<Signal, String> {
        let args = signal::CaptureArgs {
            kind: wire(&kind),
            from,
            body,
            content: None,
        };
        decode(settle(
            self.services.signal.signal_capture(Parameters(args)),
        )?)
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
        let args = signal::ResearchArgs {
            id: id.to_owned(),
            summary,
            confidence,
            sources,
            think_step,
        };
        decode(settle(
            self.services.signal.signal_research(Parameters(args)),
        )?)
    }

    /// Promote a researched or surfaced signal to the roadmap (the
    /// `signal_promote` verb): the backlog chunk `signal-<id>` is created
    /// and the two are cross-linked — `signal:<id>` on the chunk,
    /// `chunk:<id>` on the signal — idempotently. Answers the signal as it
    /// stands after.
    pub fn signal_promote(&self, id: &str) -> Result<Signal, String> {
        let args = signal::PromoteArgs { id: id.to_owned() };
        settle(self.services.signal.signal_promote(Parameters(args)))?;
        self.signal(id)
            .ok_or_else(|| format!("signal '{id}' promoted but not found"))
    }

    /// Dismiss a signal (the `signal_ignore` verb; terminal).
    pub fn signal_ignore(&self, id: &str) -> Result<Signal, String> {
        let args = signal::IgnoreArgs { id: id.to_owned() };
        decode(settle(
            self.services.signal.signal_ignore(Parameters(args)),
        )?)
    }

    /// Suppress a signal from surfacing for `minutes` (the `signal_snooze`
    /// verb); status unchanged.
    pub fn signal_snooze(&self, id: &str, minutes: i64) -> Result<Signal, String> {
        let args = signal::SnoozeArgs {
            id: id.to_owned(),
            minutes: Some(minutes),
        };
        decode(settle(
            self.services.signal.signal_snooze(Parameters(args)),
        )?)
    }
}

#[cfg(test)]
#[path = "native_engines.tests.rs"]
mod tests;
