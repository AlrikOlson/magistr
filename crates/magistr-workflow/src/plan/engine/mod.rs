//! The plan engine: owns the in-memory [`Plan`] and mediates every
//! mutation, mirroring the shape of `run::engine::RunEngine` (a plain struct
//! whose concurrency guard lives at the service layer; builder-style
//! `with_persistence`; persist-on-mutation).
//!
//! Dependency direction: this module depends on `crate::plan::domain` (pure)
//! and `crate::infra` (persistence) — never the reverse (DIP).

mod committed_events;
mod editing;
mod mutation_error;
mod observation;
mod transaction;
mod upgrade;
use magistr_workflow_contracts::Observed;
pub use mutation_error::PlanMutationError;
mod proposal_resolution;
mod readiness;

use magistr_workflow_contracts::plan::next_ready;

use chrono::Utc;

use crate::cloud::client::CloudClient;
use crate::infra::{CrossRef, Persistence, StoreStamp};
use crate::plan::broadcast::{Broadcaster, PlanFrame};
use crate::plan::domain::{
    ContainerKind, ContainerLink, FeatureCatalog, FeatureFrontier, FeatureHistoryRecords,
    FeatureResolution, Focus, FocusMode, Plan, RefreshNote, ReprioritizeProposal, Task, TaskStatus,
    TrackerLink, TrackerOptIn, WaitingOn, WaitingReason, feature_catalog, feature_frontier,
    feature_history, validate_team,
};
use crate::plan::region;

pub struct PlanEngine {
    plan: Plan,
    project_id: String,
    persistence: Option<Persistence>,
    /// The last validated or successfully written file, used only to report changes.
    stamp: Option<StoreStamp>,
    broadcaster: Option<Broadcaster>,
    /// Optional git-native trace mirror, reusing the same mirror core as the
    /// decision/run trace stores. When set, every mutation mirrors into
    /// `.magistr/traces/` as an Agent Trace
    /// JSONL record; the session commits on task completion/obsoletion. All
    /// git/IO runs on the worker's own thread, never under this engine's lock.
    /// `None` = the default Local behaviour. Fire-and-forget.
    mirror: Option<crate::infra::MirrorWorker>,
    /// Whether mirrored records are `shared` (committed `sessions/`) vs `local`
    /// (gitignored). Default `false`. Only meaningful with `mirror`.
    repo_shared: bool,
    /// Optional cloud sync client. When set, every mutation
    /// fire-and-forget pushes the task envelope to the cloud backend. `None`
    /// (default) = no cloud sync.
    cloud: Option<CloudClient>,
    /// The provider a newly-born ACTIVE task inherits an opt-in to
    /// (tracker-optin-never-grows). `None` — the default, and what every test
    /// and every unconfigured project gets — means tasks are born silent.
    ///
    /// A plain `Option<String>` rather than a `TrackerConfig` on purpose: the
    /// engine must not learn that `crate::tracker::config` exists, so the
    /// DECISION is made by [`crate::tracker::config::inherited_opt_in`] at the
    /// composition root and only its answer is handed down. That also means the
    /// engine cannot accidentally widen the rule.
    inherit_opt_in: Option<String>,
}

impl PlanEngine {
    pub fn new(project_id: String) -> Self {
        Self {
            plan: Self::empty_plan(&project_id),
            project_id,
            persistence: None,
            stamp: None,
            broadcaster: None,
            mirror: None,
            repo_shared: false,
            cloud: None,
            inherit_opt_in: None,
        }
    }

    /// The plan of a project nothing has been written to.
    fn empty_plan(project_id: &str) -> Plan {
        Plan {
            project_id: project_id.to_owned(),
            ..Plan::default()
        }
    }

    /// Arm opt-in inheritance for tasks born from here on: every new ACTIVE
    /// task is opted in to `provider`. `None` disarms it.
    ///
    /// Builder half, for a composition root that knows the answer up front
    /// (the host). Takes the resolved provider, never the config — see the
    /// `inherit_opt_in` field for why the engine is kept ignorant.
    #[must_use]
    pub fn with_opt_in_inheritance(mut self, provider: Option<String>) -> Self {
        self.set_opt_in_inheritance(provider);
        self
    }

    /// The setter half, for the moment consent is granted to an engine that is
    /// ALREADY RUNNING — `tracker setup` over MCP hands the live server's engine
    /// to `setup_local`. Without this, a setup would appear to work and then not
    /// take effect until the next restart, which is the same class of silent gap
    /// this setter exists to close.
    pub fn set_opt_in_inheritance(&mut self, provider: Option<String>) {
        self.inherit_opt_in = provider
            .map(|p| p.trim().to_ascii_lowercase())
            .filter(|p| !p.is_empty());
    }

    /// The provider new tasks inherit, if any. Reads back what was armed.
    #[must_use]
    pub fn opt_in_inheritance(&self) -> Option<&str> {
        self.inherit_opt_in.as_deref()
    }

    /// The project this engine belongs to — the reconcile filter's identity
    /// (sync-project-scope): only records stamped with this id merge in.
    #[must_use]
    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    pub fn with_broadcaster(mut self, broadcaster: Broadcaster) -> Self {
        self.set_broadcaster(broadcaster);
        self
    }

    /// Attach the admitted owner's publisher to an already-open engine.
    pub fn set_broadcaster(&mut self, broadcaster: Broadcaster) {
        self.broadcaster = Some(broadcaster);
    }

    /// Attach a cloud client so every mutation fire-and-forget pushes the task
    /// envelope to the cloud backend. Wired by the host's composition root.
    pub fn with_cloud(mut self, client: CloudClient) -> Self {
        self.cloud = Some(client);
        self
    }

    /// Attach a git-native trace sink. `shared` selects the
    /// committed `sessions/` partition (`true`) vs the gitignored `local/`
    /// partition (`false`). Wired by the host's composition root.
    pub fn with_repo_sink(mut self, sink: crate::infra::RepoSink, shared: bool) -> Self {
        self.mirror = Some(crate::infra::MirrorWorker::spawn(sink));
        self.repo_shared = shared;
        self
    }

    /// Block until the git-native mirror has drained every mutation submitted so
    /// far. No-op without a mirror. For graceful shutdown and deterministic tests.
    pub fn flush_mirror(&self) {
        if let Some(m) = &self.mirror {
            m.flush();
        }
    }

    /// Fire-and-forget: mirror the mutation into the git-native trace, then
    /// fan it out to the broadcast socket. A sink/broadcast error is
    /// logged at WARN and dropped — a mutation is never failed by it. The
    /// frame→record mapping lives here (engine-side) so `infra::repo_sync`
    /// stays domain-free.
    fn record_event(&self, frame: PlanFrame) {
        self.mirror_frame_to_repo(&frame);
        if let Some(b) = &self.broadcaster {
            b.emit(&frame);
        }
        self.cloud_push_frame(&frame);
    }

    /// Fire-and-forget: push the frame's task envelope to the cloud backend.
    /// No-op without a cloud client OR outside a tokio runtime
    /// (so sync unit tests never panic); a push error is logged and dropped, so
    /// a mutation is never failed by it.
    fn cloud_push_frame(&self, frame: &PlanFrame) {
        let Some(client) = &self.cloud else {
            return;
        };
        let task = match frame {
            PlanFrame::TaskAdded { task }
            | PlanFrame::TaskChanged { task }
            | PlanFrame::TaskCompleted { task }
            | PlanFrame::TaskRetired { task } => task,
            PlanFrame::RefreshRecorded { .. } => return,
        };
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            return;
        };
        // The task's tracker state rides its envelope (see build::from_task),
        // so a peer inherits the binding instead of minting a second twin.
        let links: Vec<_> = self
            .plan
            .links
            .iter()
            .filter(|l| l.task_id == task.id)
            .cloned()
            .collect();
        let opt_ins: Vec<_> = self
            .plan
            .tracker_opt_ins
            .iter()
            .filter(|o| o.task_id == task.id)
            .cloned()
            .collect();
        let envelope = crate::cloud::build::from_task(&self.project_id, task, &links, &opt_ins);
        let client = client.clone();
        handle.spawn(async move {
            if let Err(e) = client.push(&envelope).await {
                tracing::warn!(target: "magistr_workflow::cloud", "plan cloud push failed: {e}");
            }
        });
    }

    fn mirror_frame_to_repo(&self, frame: &PlanFrame) {
        let Some(mirror) = &self.mirror else {
            return;
        };
        let (kind, closes) = frame.record_meta();
        let payload = match frame {
            PlanFrame::TaskAdded { task }
            | PlanFrame::TaskChanged { task }
            | PlanFrame::TaskCompleted { task }
            | PlanFrame::TaskRetired { task } => {
                serde_json::to_value(task).unwrap_or(serde_json::Value::Null)
            }
            PlanFrame::RefreshRecorded { summary, notes } => {
                serde_json::json!({ "summary": summary, "notes": notes })
            }
        };

        // Hand off to the worker thread; building the record (which shells git),
        // the append, and any commit all happen off this engine's lock.
        mirror.submit(crate::infra::MirrorJob {
            family: "plan",
            kind,
            session_id: self.project_id.clone(),
            shared: self.repo_shared,
            payload,
            files: vec![],
            closes,
        });
    }

    /// Attach the selected store and load it without rewriting authored records.
    pub fn with_persistence(mut self, persistence: Persistence) -> Self {
        self.persistence = Some(persistence);
        if let Err(error) = self.reload_from_store() {
            tracing::warn!(%error, "The plan could not be loaded");
        }
        self
    }

    /// Whether this engine commits to a file rather than memory.
    #[must_use]
    pub fn persistence_enabled(&self) -> bool {
        self.persistence.as_ref().is_some_and(Persistence::enabled)
    }

    /// Validate the selected file and adopt its records before acting.
    /// Answers whether the observation changed the held records or source.
    pub fn refresh_if_stale(&mut self) -> Result<bool, String> {
        self.refresh_observed().map_err(|error| error.to_string())
    }

    /// Explicitly validate and adopt the selected persisted plan.
    pub fn reload_from_store(&mut self) -> Result<bool, String> {
        self.refresh_observed().map_err(|error| error.to_string())
    }

    fn now() -> String {
        Utc::now().to_rfc3339()
    }

    /// Read-only access for the wire/family layer.
    pub fn plan(&self) -> &Plan {
        &self.plan
    }

    /// Merge a task pulled from the cloud: insert when absent;
    /// replace an existing copy only when the incoming one is strictly NEWER
    /// (`domain::task_wins` — the same updated_at rule as the disk merge).
    /// Cloud-wins-unconditionally was the original policy and clobbered fresh
    /// local mutations when a realtime refresh raced their own push
    /// (reconcile-recency-guard). A SILENT merge — it does NOT
    /// emit a mutation frame or push back to the cloud; a reconcile that
    /// re-emitted would loop the pull straight back into a push. Persists.
    pub fn upsert_task(&mut self, task: Task) -> Result<Observed<bool>, PlanMutationError> {
        self.upsert_task_with_tracker(task, Vec::new(), Vec::new())
    }

    pub fn upsert_task_with_tracker(
        &mut self,
        task: Task,
        links: Vec<TrackerLink>,
        opt_ins: Vec<TrackerOptIn>,
    ) -> Result<Observed<bool>, PlanMutationError> {
        self.mutate(|edit| {
            let adopted = edit.upsert_task(task)?;
            let sidecars = edit.adopt_tracker_state(links, opt_ins)?;
            Ok(adopted || sidecars > 0)
        })
    }

    /// Add a new task. Errors on a duplicate id.
    #[allow(clippy::too_many_arguments)]
    pub fn add_task(
        &mut self,
        id: String,
        title: String,
        status: TaskStatus,
        priority: u32,
        description: String,
        acceptance: Vec<String>,
        deps: Vec<String>,
        shared: bool,
    ) -> Result<Observed<Task>, PlanMutationError> {
        self.mutate(|edit| {
            edit.add_task(
                id,
                title,
                status,
                priority,
                description,
                acceptance,
                deps,
                shared,
            )
            .cloned()
        })
    }

    /// [`Self::add_task`] carrying the optional structured body — the rich
    /// rendering sidecar the MCP seam steers writers toward. Validated by the
    /// caller; `None` is every legacy writer.
    #[allow(clippy::too_many_arguments)]
    pub fn add_task_with_content(
        &mut self,
        id: String,
        title: String,
        name: String,
        status: TaskStatus,
        priority: u32,
        description: String,
        acceptance: Vec<String>,
        deps: Vec<String>,
        shared: bool,
        content: Option<crate::content::StructuredContent>,
        feature: Option<String>,
    ) -> Result<Observed<Task>, PlanMutationError> {
        self.mutate(|edit| {
            edit.add_task_with_content(
                id,
                title,
                name,
                status,
                priority,
                description,
                acceptance,
                deps,
                shared,
                content,
                feature,
            )
            .cloned()
        })
    }

    /// Move a task to a new status, validated against the transition table.
    ///
    /// An explicit transition also DISPOSES any open status proposal — the
    /// field-scoped disposal rule ("Proposals, and what disposes them", above
    /// [`Self::propose_status`]). Moving to the suggested status accepts the
    /// suggestion; moving anywhere else supersedes it. Either way the human
    /// acted on the field, which is all a proposal ever asks for.
    pub fn set_status(
        &mut self,
        id: &str,
        to: TaskStatus,
    ) -> Result<Observed<Task>, PlanMutationError> {
        self.mutate(|edit| edit.set_status(id, to).cloned())
    }

    /// Patch a task's descriptive fields (status is handled by `set_status`).
    /// Any `None` argument leaves that field unchanged — including `content`,
    /// which is replace-only here (like `title`); nothing needs to clear it.
    #[allow(clippy::too_many_arguments)]
    pub fn update_task(
        &mut self,
        id: &str,
        title: Option<String>,
        priority: Option<u32>,
        description: Option<String>,
        acceptance: Option<Vec<String>>,
        deps: Option<Vec<String>>,
        content: Option<crate::content::StructuredContent>,
    ) -> Result<Observed<Task>, PlanMutationError> {
        self.mutate(|edit| {
            edit.update_task(id, title, priority, description, acceptance, deps, content)
                .cloned()
        })
    }

    /// Put a task in a feature, or take it out of one.
    ///
    /// A verb of its own rather than a seventh `Option` on [`Self::update_task`],
    /// for a reason that is not style: that function reads `None` as "leave
    /// alone", so it cannot express CLEARING a field. Grouping needs both, and
    /// `Option<Option<String>>` to get them would be a worse interface than a
    /// named method.
    ///
    /// An empty or whitespace-only name clears rather than creating a container
    /// called "" upstream.
    ///
    /// A name that repeats a task id prefix is REFUSED here rather than left
    /// for `doctor` to object to later. The engine holds every task, so it is
    /// the only place that can tell a caller at the moment it happens; the
    /// alternative was a red diagnostic hours afterwards with no memory of who
    /// wrote it. The phrase returned is the one [`region::why_unfit`] prints.
    /// Validate and update a task’s feature in its owning transaction.
    pub fn set_feature(
        &mut self,
        id: &str,
        feature: Option<String>,
    ) -> Result<Observed<Task>, PlanMutationError> {
        self.mutate(|edit| edit.set_feature(id, feature).cloned())
    }

    /// Rewrite the short label a node wears, leaving the sentence `title`
    /// alone.
    ///
    /// Its own verb rather than another argument on [`Self::update_task`],
    /// which already carries seven and is called from twenty-five places. This
    /// mirrors [`Self::set_feature`], the other field a human corrects one at a
    /// time.
    ///
    /// Passing an empty name re-seeds from the id rather than clearing it: a
    /// nameless task is exactly what this field exists to make unreachable.
    pub fn set_name(&mut self, id: &str, name: &str) -> Result<Observed<Task>, PlanMutationError> {
        self.mutate(|edit| edit.set_name(id, name).cloned())
    }

    /// Every task whose name cannot serve as a node label, with the reason.
    /// The data-side half of constraint C8 — `doctor` reports it, and it is the
    /// check that stops the sentence style leaking back in.
    #[must_use]
    pub fn tasks_without_usable_names(&self) -> Vec<(String, String)> {
        self.plan
            .tasks
            .iter()
            .filter_map(|c| crate::plan::name::why_unfit(&c.name).map(|why| (c.id.clone(), why)))
            .collect()
    }

    /// Current work and terminal history from this complete owner snapshot.
    #[must_use]
    pub fn features(&self) -> FeatureCatalog {
        feature_catalog(&self.plan.tasks)
    }

    // ── Focus: what one team is currently working on ───────────────────
    //
    // Every verb below is keyed by team. There is deliberately no team-less
    // variant of any of them: a convenience overload defaulting the team is
    // the project-global focus this design exists to make impossible, and it
    // would be one commit away at all times if the door were left open.

    /// What `team` is focused on, or `None` if it has never focused anything.
    ///
    /// Read-only, and it does not validate the team: asking after a nonsense
    /// team is honestly answered "that team has no focus", which is true and is
    /// what a caller reconstructing its own state needs to hear.
    #[must_use]
    pub fn focus_get(&self, team: &str) -> Option<&Focus> {
        let team = team.trim();
        self.plan.focuses.iter().find(|f| f.team == team)
    }

    /// Point `team` at `feature` in `mode`.
    ///
    /// Validates BEFORE it writes, and the ordering is the contract: a call
    /// naming a closed, unknown or ambiguous feature leaves the store unchanged,
    /// so a mistyped switch can never strand a team somewhere it did not ask to
    /// be. Task status is untouched — focus points at work, it does not start
    /// any.
    pub fn focus_set(
        &mut self,
        team: &str,
        feature: &str,
        mode: FocusMode,
    ) -> Result<Observed<Focus>, PlanMutationError> {
        self.mutate(|edit| edit.focus_set(team, feature, mode))
    }

    /// Drop `team`'s focus. `true` when there was one to drop.
    pub fn focus_clear(&mut self, team: &str) -> Result<Observed<bool>, PlanMutationError> {
        self.mutate(|edit| edit.focus_clear(team))
    }

    /// The next ready task INSIDE `feature` — same three readiness conditions as
    /// [`Self::next`], with membership added.
    ///
    /// It cannot return a task from another feature, and that is the property the
    /// whole focused loop rests on: an empty answer here means "nothing ready in
    /// this feature", never "look elsewhere". Callers are expected to stop.
    #[must_use]
    pub fn next_in_feature(&self, feature: &str) -> Option<&Task> {
        next_ready(
            self.plan
                .tasks
                .iter()
                .filter(|c| c.feature.as_deref() == Some(feature)),
            |c| self.readiness(c).is_ready(),
            |c| c.priority,
        )
    }

    /// Current work in exactly one feature; None selects unassigned work.
    /// Dependency readiness is assessed against every task in this owner.
    #[must_use]
    pub fn feature_frontier(&self, feature: Option<&str>) -> FeatureFrontier {
        feature_frontier(&self.plan.tasks, feature)
    }

    /// Read terminal record IDs only for the history being inspected.
    #[must_use]
    pub fn feature_history(&self, feature: Option<&str>) -> FeatureHistoryRecords {
        feature_history(&self.plan.tasks, feature)
    }

    /// Which unfinished, unassigned tasks share a prefix, offered to be named.
    ///
    /// This used to assign the prefix as the feature, and could not be repaired
    /// into doing so correctly. [`region::why_unfit`] rejects a region name
    /// contained in any task id prefix, and a prefix contains itself, so every
    /// name that guess could produce is rejected — always, not on unlucky
    /// slugs. That is where this plan's `saas`, `line`, `signal` and `iac`
    /// came from, and a fresh project running the seeder walked into the same
    /// red diagnostic with no account of what it had done.
    ///
    /// So the grouping survives and the naming does not. Read-only by
    /// consequence rather than by preference: there is nothing left it could
    /// honestly write.
    ///
    /// The floor is unchanged and still the point: measured on this plan, 17
    /// prefixes covered 239 of 317 tasks while 78 sat in a tail of 56 prefixes
    /// used once or twice, and a container per one-off slug is worse than no
    /// grouping at all.
    #[must_use]
    pub fn propose_features_from_ids(&self, min_shared: usize) -> Vec<region::Cluster> {
        region::cluster_by_prefix(
            self.plan
                .tasks
                .iter()
                .filter(|c| c.status.is_active() && c.feature.is_none())
                .map(|c| (c.id.as_str(), c.feature.as_deref())),
            min_shared,
        )
    }

    /// Mark a task retired with a reason. Kept for history, never `next()`-ed.
    pub fn retire_task(
        &mut self,
        id: &str,
        reason: String,
    ) -> Result<Observed<Task>, PlanMutationError> {
        self.mutate(|edit| edit.retire_task(id, reason).cloned())
    }

    // ── Proposals, and what disposes them ──────────────────────────────────
    //
    // Three proposal types live on a task, and all three obey ONE disposal
    // rule: a proposal is disposed by the human's explicit act on the field it
    // proposes about. Nothing else clears it, and nothing needs to — the act
    // the proposal was soliciting is itself the disposal.
    //
    // | Proposal      | Set by                    | Disposed by                                   |
    // |---------------|---------------------------|-----------------------------------------------|
    // | status        | propose_status            | set_status / retire_task (any transition)  |
    // | title         | propose_title             | resolve_title_proposal, or a differing title  |
    // |               |                           | edit in update_task                          |
    // | reprioritize  | propose_reprioritize      | resolve_reprioritize or a differing edit      |
    //
    // Explicit native resolution settles each proposal under one mutation
    // lock. Rejection and an accepted same-priority suggestion must clear the
    // priority proposal without publishing a temporary priority detour.
    //
    // Eager disposal is safe because proposals REGENERATE: the sweep re-mints
    // on its next cycle while a divergence persists, so a disposal costs at
    // most one sweep interval, while a proposal that nothing clears costs the
    // truthfulness of every status view forever (the defect
    // tracker-status-proposal-disposal was filed about).
    //
    // Disposal is FIELD-SCOPED on purpose: a transition does not touch a title
    // proposal — a contested title stays contested when its task completes,
    // and the projector may be actively honoring that concession.

    /// Record a STATUS proposal. Never changes the task's real `status`.
    ///
    /// The mirror of [`Self::propose_reprioritize`], deliberately: reusing the
    /// established shape means a human already knows what a proposal is and
    /// where to look for it, instead of learning a second mechanism that does
    /// the same job with different words.
    ///
    /// A proposal identical to the one already recorded is a no-op, so a sweep
    /// that runs every hour does not restamp `proposed_at` forever and make an
    /// old suggestion look perpetually new.
    pub fn propose_status(
        &mut self,
        id: &str,
        suggested_status: TaskStatus,
        reason: String,
        source: String,
    ) -> Result<Observed<Task>, PlanMutationError> {
        self.mutate(|edit| {
            edit.propose_status(id, suggested_status, reason, source)
                .cloned()
        })
    }

    /// Record a conceded contested TITLE. Never changes the task's real
    /// `title` — but unlike its proposal siblings it is CONSULTED: while open,
    /// the projector keeps sending `suggested_title` instead of re-asserting
    /// the plan's, which is what makes the concession durable
    /// (tracker-contested-memory).
    ///
    /// Idempotent the same way [`Self::propose_status`] is: an identical
    /// suggestion is a no-op, so a projection running every few minutes does
    /// not restamp `proposed_at` and make an old disagreement look new. A
    /// remote that moves AGAIN to a different title is a different
    /// disagreement and replaces the proposal.
    pub fn propose_title(
        &mut self,
        id: &str,
        suggested_title: String,
        reason: String,
        source: String,
    ) -> Result<Observed<Task>, PlanMutationError> {
        self.mutate(|edit| {
            edit.propose_title(id, suggested_title, reason, source)
                .cloned()
        })
    }

    /// Resolve an open title proposal — the human act with both outcomes.
    ///
    /// `accept: true` adopts the tracker's title into the plan and clears the
    /// proposal; `accept: false` clears it so the plan's title flows again on
    /// the next projection. Erring on a task with no open proposal is loud
    /// rather than a silent no-op: "resolved" must mean something happened.
    pub fn resolve_title_proposal(
        &mut self,
        id: &str,
        accept: bool,
    ) -> Result<Observed<Task>, PlanMutationError> {
        self.mutate(|edit| edit.resolve_title_proposal(id, accept).cloned())
    }

    /// Record a re-prioritization *proposal*. This never changes the task's
    /// real `priority` — it only attaches a suggestion for a human to accept,
    /// keeping "never auto-reorder" a type-level invariant.
    ///
    /// Idempotent the same way its two siblings are: an identical suggestion
    /// is a no-op, so a repeated proposal does not restamp `proposed_at` and
    /// make an old suggestion look perpetually new. It was the one sibling
    /// WITHOUT this guard — an accidental asymmetry, ended when the disposal
    /// rules were unified (tracker-status-proposal-disposal).
    pub fn propose_reprioritize(
        &mut self,
        id: &str,
        suggested_priority: u32,
        reason: String,
    ) -> Result<Observed<Task>, PlanMutationError> {
        self.mutate(|edit| {
            edit.propose_reprioritize(id, suggested_priority, reason)
                .cloned()
        })
    }

    // ── Cross-family integration ────────────────────────────────────────────
    //
    // These keep the families decoupled: the plan records links and hands
    // back the `task:<id>` reference for the agent to wire into run/decision. No
    // plan tool reaches into the run or decision engines.

    /// Start eligible pending work; recheck its current blockers and dependencies. Returns the
    /// task; the caller derives the `task:<id>` reference to pass to
    /// `run_open` / a decision `run_ref`.
    pub fn start_task(&mut self, id: &str) -> Result<Observed<Task>, PlanMutationError> {
        self.mutate(|edit| edit.start_task(id).cloned())
    }

    /// Mark a task done, optionally attaching the proof-of-run cross-ref
    /// (e.g. `step:<id>` or `check:<name>`). The ref is validated and stored
    /// in normalized form.
    pub fn complete_task(
        &mut self,
        id: &str,
        step_ref: Option<&str>,
    ) -> Result<Observed<Task>, PlanMutationError> {
        self.mutate(|edit| edit.complete_task(id, step_ref).cloned())
    }

    /// Attach any validated cross-reference (`decision:N`, `step:X`, `progress:N`,
    /// `check:X`, `task:X`) to a task. Duplicates are ignored.
    pub fn link_task(
        &mut self,
        id: &str,
        cross_ref: &str,
    ) -> Result<Observed<Task>, PlanMutationError> {
        self.mutate(|edit| edit.link_task(id, cross_ref).cloned())
    }

    /// Bind a task to its twin in an external tracker, or update the binding
    /// after a write (tracker-port-seam).
    ///
    /// Upserts by `(task_id, provider)` — one task has at most one twin per
    /// provider — and stamps the content hash and version that later make an
    /// idempotent re-projection and an echo fence possible. Also attaches the
    /// `ext:<provider>/<id>` cross-ref to the task, so the binding is visible
    /// in the same provenance graph as `decision:`/`step:` refs rather than in a
    /// side table only this module knows about.
    ///
    /// `content_hash` is `WorkItem::content_hash()` of the payload just written.
    pub fn record_tracker_link(
        &mut self,
        task_id: &str,
        provider: &str,
        external_id: &str,
        content_hash: &str,
        version: Option<&str>,
    ) -> Result<Observed<TrackerLink>, PlanMutationError> {
        self.mutate(|edit| {
            edit.record_tracker_link(task_id, provider, external_id, content_hash, version)
                .cloned()
        })
    }

    /// Bind a container (feature or roof) to its upstream twin, or refresh the
    /// binding after a push (tracker-group-ownership).
    ///
    /// Upserts by `(kind, name, provider)`. `created_by_us` is STICKY: once a
    /// binding remembers that we minted the container, a later resolve must not
    /// erase that fact, because it is what a future cleanup uses to tell our
    /// empty containers from a human's.
    pub fn record_container_link(
        &mut self,
        kind: ContainerKind,
        name: &str,
        provider: &str,
        external_id: &str,
        created_by_us: bool,
    ) -> Result<Observed<()>, PlanMutationError> {
        self.mutate(|edit| {
            edit.record_container_link(kind, name, provider, external_id, created_by_us)
        })
    }

    /// The container's upstream binding, if one was ever recorded.
    #[must_use]
    pub fn container_link(
        &self,
        kind: ContainerKind,
        name: &str,
        provider: &str,
    ) -> Option<&ContainerLink> {
        self.plan
            .container_links
            .iter()
            .find(|c| c.kind == kind && c.name == name && c.provider == provider)
    }

    /// The task's twin on `provider`, if it has one.
    #[must_use]
    pub fn tracker_link(&self, task_id: &str, provider: &str) -> Option<&TrackerLink> {
        let provider = provider.trim().to_ascii_lowercase();
        self.plan
            .links
            .iter()
            .find(|l| l.task_id == task_id && l.provider == provider)
    }

    /// The link that binds `external_id` on `provider` back to a task.
    ///
    /// The inbound direction of [`Self::tracker_link`]: a sweep or a webhook
    /// knows the provider's id and needs to find out whether the item is ours.
    /// Deliberately searches ALL links rather than only opted-in tasks — a
    /// task opted OUT after projection still owns its twin, and forgetting
    /// that would make our own writes read as remote changes forever.
    #[must_use]
    pub fn tracker_link_by_external_id(
        &self,
        provider: &str,
        external_id: &str,
    ) -> Option<&TrackerLink> {
        let provider = provider.trim().to_ascii_lowercase();
        self.plan
            .links
            .iter()
            .find(|l| l.provider == provider && l.external_id == external_id)
    }

    /// Every tracker binding a task has, across providers.
    #[must_use]
    pub fn tracker_links_for(&self, task_id: &str) -> Vec<&TrackerLink> {
        self.plan
            .links
            .iter()
            .filter(|l| l.task_id == task_id)
            .collect()
    }

    /// Record the fingerprint of the blocking-link set last declared for this
    /// task on `provider`. Kept apart from [`Self::record_tracker_link`] so the
    /// content fence and the relation fence move independently: a projection can
    /// change content without changing deps, and the reverse.
    ///
    /// Fails when no link exists yet — relations are declared between items that
    /// have already been created, so a link is always the prerequisite.
    pub fn record_tracker_relations(
        &mut self,
        task_id: &str,
        provider: &str,
        relations_hash: &str,
    ) -> Result<Observed<TrackerLink>, PlanMutationError> {
        self.mutate(|edit| {
            edit.record_tracker_relations(task_id, provider, relations_hash)
                .cloned()
        })
    }

    /// Record the digest of just the fields we AUTHORED in the write that
    /// [`Self::record_tracker_link`] just stamped.
    ///
    /// A sibling setter rather than a sixth parameter on `record_tracker_link`,
    /// for the reason [`Self::record_tracker_relations`] is one: the fences move
    /// independently and every existing caller keeps compiling. It also means an
    /// adapter path that records a link without an authored digest leaves `None`
    /// — which the preview reads as "cannot tell", the honest answer, rather
    /// than as a confident wrong one.
    ///
    /// Fails when no link exists yet: the authored digest describes a write, and
    /// a write always produces the link first.
    pub fn record_tracker_authored(
        &mut self,
        task_id: &str,
        provider: &str,
        authored_hash: &str,
    ) -> Result<Observed<TrackerLink>, PlanMutationError> {
        self.mutate(|edit| {
            edit.record_tracker_authored(task_id, provider, authored_hash)
                .cloned()
        })
    }

    /// Opt a task in or out of projection into `provider`. Opting out is
    /// recorded rather than deleted so an explicit refusal wins the recency
    /// merge against a peer still holding the older opt-in.
    pub fn set_tracker_opt_in(
        &mut self,
        task_id: &str,
        provider: &str,
        enabled: bool,
    ) -> Result<Observed<TrackerOptIn>, PlanMutationError> {
        self.mutate(|edit| edit.set_tracker_opt_in(task_id, provider, enabled).cloned())
    }

    /// Whether this task has explicitly consented to `provider`. Absent means
    /// no — silence is the default.
    #[must_use]
    pub fn tracker_opt_in(&self, task_id: &str, provider: &str) -> Option<&TrackerOptIn> {
        let provider = provider.trim().to_ascii_lowercase();
        self.plan
            .tracker_opt_ins
            .iter()
            .find(|o| o.task_id == task_id && o.provider == provider)
    }

    /// Adopt tracker state that arrived from another machine inside a task
    /// envelope (see `cloud::build::from_task`).
    ///
    /// Merged by `(task_id, provider)` on strict `updated_at` recency — the
    /// same rule [`crate::plan::domain::merge_plans`] uses on disk, so a
    /// record never means one thing over the wire and another on the filesystem.
    /// A stale peer copy cannot clobber a fresher local one. Returns how many
    /// records were actually adopted.
    pub fn adopt_tracker_state(
        &mut self,
        links: Vec<TrackerLink>,
        opt_ins: Vec<TrackerOptIn>,
    ) -> Result<Observed<usize>, PlanMutationError> {
        self.mutate(|edit| edit.adopt_tracker_state(links, opt_ins))
    }

    /// The task ids opted in to `provider`, in plan order so a projection
    /// run is deterministic.
    #[must_use]
    pub fn tasks_opted_in(&self, provider: &str) -> Vec<&Task> {
        let provider = provider.trim().to_ascii_lowercase();
        self.plan
            .tasks
            .iter()
            .filter(|c| {
                self.plan
                    .tracker_opt_ins
                    .iter()
                    .any(|o| o.task_id == c.id && o.provider == provider && o.enabled)
            })
            .collect()
    }

    /// THE DRIFT: active tasks that are NOT opted in to `provider`, in plan
    /// order.
    ///
    /// The complement of [`Self::tasks_opted_in`], and it exists because the
    /// count that was reported for a year was the wrong one. `tracker status`
    /// said "46 items included" every day while the plan grew to 354 tasks,
    /// and 46 is a perfectly reassuring number — nothing said that 20 active
    /// tasks were invisible. A scope that can silently stop growing needs a
    /// readout of what it is NOT covering, or the next gap is found the same way
    /// this one was: by a human noticing months later that the tracker is stale.
    ///
    /// Done and retired tasks are not drift — they are deliberately out of
    /// scope ([`TaskStatus::is_active`]).
    #[must_use]
    pub fn tasks_not_opted_in(&self, provider: &str) -> Vec<&Task> {
        let provider = provider.trim().to_ascii_lowercase();
        self.plan
            .tasks
            .iter()
            .filter(|c| c.status.is_active())
            .filter(|c| {
                !self
                    .plan
                    .tracker_opt_ins
                    .iter()
                    .any(|o| o.task_id == c.id && o.provider == provider && o.enabled)
            })
            .collect()
    }

    /// Append a datestamped refresh note recording a plan mutation and the
    /// decision note ids that motivated it. Returns the new note count.
    pub fn record_refresh(
        &mut self,
        summary: String,
        notes: Vec<u32>,
    ) -> Result<Observed<usize>, PlanMutationError> {
        self.mutate(|edit| edit.record_refresh(summary, notes))
    }

    /// Validate + normalize a wire cross-ref string via [`CrossRef`].
    fn normalize_ref(&self, raw: &str) -> Result<String, String> {
        CrossRef::from_wire(raw)
            .map(|r| r.to_wire())
            .map_err(|e| format!("invalid cross-ref '{raw}': {e}"))
    }

    /// Build a validated [`WaitingOn`], or say exactly what is wrong with it.
    ///
    /// This lives in the engine rather than on the type because
    /// `plan::domain` is pure data with no `crate::infra` imports (its module
    /// header states the rule), and [`CrossRef`] is infra. Rather than write a
    /// second parser for a wire format that already has one — which would drift
    /// the moment either changed — the domain stores `evidence` as a String and
    /// this reuses the same `normalize_ref` helper `refs` goes through,
    /// so both fields accept exactly the same vocabulary.
    ///
    /// Two refusals, both about the same failure. A blank `reason` is rejected
    /// because a blocker nobody wrote a reason for is the prose-in-the-title
    /// problem wearing a struct; an unparseable `evidence` is rejected because a
    /// ref that resolves to nothing is worse than the honest `None`.
    ///
    /// Note this VALIDATES and does not store: attaching a blocker to a task,
    /// and clearing it again, is `blocked-by-set-and-cleared`. Keeping the
    /// stored-schema change and the write path in separate tasks is what lets
    /// each be verified on its own.
    pub fn validate_waiting_on(
        &self,
        kind: WaitingReason,
        reason: String,
        evidence: Option<String>,
    ) -> Result<WaitingOn, String> {
        if reason.trim().is_empty() {
            return Err(
                "a waiting reason needs a sentence: the point of waiting_on is that nobody \
                 has to re-derive it from the title"
                    .to_string(),
            );
        }
        let evidence = evidence
            .map(|raw| self.normalize_ref(&raw))
            .transpose()
            .map_err(|e| format!("invalid waiting evidence: {e}"))?;
        Ok(WaitingOn {
            kind,
            reason: reason.trim().to_string(),
            evidence,
            waiting_since: Self::now(),
        })
    }

    /// Attach a blocker to a task, replacing any blocker already there.
    ///
    /// Validation happens in [`Self::validate_waiting_on`] and is not repeated
    /// here: a caller that has already built a `WaitingOn` has been through the
    /// refusals, and re-checking would be a second place for the rules to drift.
    ///
    /// Replacing rather than refusing an existing blocker is deliberate. The
    /// reason a task is stuck changes — a premise that was merely unmet gets
    /// refuted, an external wait becomes a wait on a person — and making the
    /// re-statement of a blocker require a clear first would put a gesture
    /// between a human and the truth, which is how records go stale.
    pub fn set_waiting_on(
        &mut self,
        id: &str,
        waiting_on: WaitingOn,
    ) -> Result<Observed<Task>, PlanMutationError> {
        self.mutate(|edit| edit.set_waiting_on(id, waiting_on).cloned())
    }

    /// Retract a task's blocker — the half that decides whether the other half
    /// gets used at all.
    ///
    /// Every blocker eventually becomes wrong: the human answers, the tenant
    /// appears, the refuted premise is re-tested and holds. If saying so is
    /// harder than recording it in the first place, the field rots into the
    /// stale prose it was built to replace, so this is a first-class verb and
    /// not a special case of the setter.
    ///
    /// Clearing removes the key entirely rather than storing an emptied husk,
    /// so a task that was unblocked is indistinguishable on disk from one that
    /// was never blocked — the property `clearing_leaves_no_trace_on_disk`
    /// asserts exactly that.
    ///
    /// Erring on a task with no blocker is loud, matching
    /// [`Self::resolve_title_proposal`]: "cleared" has to mean something
    /// happened, and a caller who believes there is a blocker to retract when
    /// there is none is working from a stale picture worth interrupting.
    pub fn clear_waiting_on(&mut self, id: &str) -> Result<Observed<Task>, PlanMutationError> {
        self.mutate(|edit| edit.clear_waiting_on(id).cloned())
    }

    /// The next task to work: the lowest-`priority` `ToDo` task that
    /// carries no blocker and whose dependencies are all `Done`. Ties break by
    /// insertion order. `None` when nothing is ready.
    ///
    /// Three conditions, and the middle one is the youngest. An unfinished
    /// dependency used to be the only way the scheduler could refuse a task,
    /// so anything else it should not pick up had to be hidden by demoting its
    /// status — and a demoted task loses the ranking that said how much it
    /// mattered. A [`Task::waiting_on`] task is SKIPPED, not hidden: it keeps
    /// its priority, its band and its place in [`Self::status`], and is simply
    /// never the answer to "what should I do next".
    pub fn next(&self) -> Option<&Task> {
        next_ready(
            &self.plan.tasks,
            |c| self.readiness(c).is_ready(),
            |c| c.priority,
        )
    }

    /// A JSON snapshot: status counts, the next-ready task, and the **actionable**
    /// task list. Deliberately bounded so the result never exceeds the MCP
    /// tool-output limit on a large plan (a real 294-task plan
    /// serialized to ~120 KB and `plan_status` hard-errored, leaving the agent
    /// blind). The list carries only active tasks (idea/to_do/in_progress/
    /// blocked) with truncated titles, capped at `Self::STATUS_LIST_CAP`; done
    /// and retired tasks appear only in `counts` plus a short `recent_done`
    /// tail. Any elision is reported (`omitted_active`, `note`) — never silent.
    pub fn status(&self) -> serde_json::Value {
        Self::status_of(&self.plan)
    }

    /// The canonical status projection of one complete plan observation.
    pub fn status_of(plan: &Plan) -> serde_json::Value {
        use TaskStatus::*;
        let count = |s: TaskStatus| plan.tasks.iter().filter(|c| c.status == s).count();

        let summary = |c: &Task| {
            let mut row = serde_json::json!({
                "id": c.id,
                "title": Self::truncate_title(&c.title),
                "name": c.name,
                "status": c.status,
                "priority": c.priority,
                "band": crate::infra::coerce::priority_band(c.priority),
                "deps": c.deps,
                "has_reprioritize_proposal": c.reprioritize.is_some(),
                "has_status_proposal": c.status_proposal.is_some(),
                "has_title_proposal": c.title_proposal.is_some(),
            });
            if let Some(b) = &c.waiting_on {
                row["waiting_on"] = serde_json::json!(b.kind.as_wire());
            }
            if let Some(g) = &c.feature {
                row["feature"] = serde_json::json!(g);
            }
            row
        };

        let mut active: Vec<&Task> = plan
            .tasks
            .iter()
            .filter(|c| matches!(c.status, Idea | ToDo | InProgress | Waiting))
            .collect();
        active.sort_by_key(|c| c.priority);
        let active_total = active.len();
        let omitted_active = active_total.saturating_sub(Self::STATUS_LIST_CAP);
        let tasks: Vec<_> = active
            .iter()
            .take(Self::STATUS_LIST_CAP)
            .map(|c| summary(c))
            .collect();

        let mut finished: Vec<&Task> = plan
            .tasks
            .iter()
            .filter(|c| matches!(c.status, Done | Retired))
            .collect();
        finished.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        let recent_done: Vec<_> = finished
            .iter()
            .take(Self::STATUS_RECENT_DONE)
            .map(|c| summary(c))
            .collect();

        let mut note = String::new();
        if omitted_active > 0 {
            note = format!(
                "{omitted_active} more active task(s) not shown (cap {}); call plan_export for the full list.",
                Self::STATUS_LIST_CAP
            );
        }

        let mut waiting_on = serde_json::Map::new();
        for kind in WaitingReason::ALL {
            waiting_on.insert(kind.as_wire().to_string(), serde_json::json!(0));
        }
        let mut waiting_on_total: u64 = 0;
        for c in active.iter().filter(|c| c.waiting_on.is_some()) {
            let kind = c.waiting_on.as_ref().expect("filtered on is_some").kind;
            let slot = waiting_on
                .get_mut(kind.as_wire())
                .expect("every kind was seeded from WaitingReason::ALL");
            *slot = serde_json::json!(slot.as_u64().unwrap_or(0) + 1);
            waiting_on_total += 1;
        }
        waiting_on.insert("total".to_string(), serde_json::json!(waiting_on_total));

        serde_json::json!({
            "project_id": plan.project_id,
            "counts": {
                "idea": count(Idea),
                "to_do": count(ToDo),
                "in_progress": count(InProgress),
                "waiting": count(Waiting),
                "done": count(Done),
                "retired": count(Retired),
                "total": plan.tasks.len(),
                "waiting_on": waiting_on,
            },
            "next": next_ready(&plan.tasks, |c| c.readiness(|id| plan.tasks.iter().find(|dependency| dependency.id == id).map(|dependency| dependency.status)).is_ready(), |c| c.priority).map(|c| c.id.clone()),
            "active_total": active_total,
            "omitted_active": omitted_active,
            "tasks": tasks,
            "recent_done": recent_done,
            "note": note,
        })
    }

    pub fn export(&self, format: &str) -> String {
        Self::export_of(&self.plan, format)
    }

    /// Render one complete observation through the canonical plan exporter.
    pub fn export_of(plan: &Plan, format: &str) -> String {
        match format {
            "json" => serde_json::to_string_pretty(plan).unwrap_or_default(),
            _ => Self::export_markdown(plan),
        }
    }

    /// Render a PLAN.md-shaped markdown *view* of the plan: tasks grouped
    /// under status sections, priority-ordered within each. This is a generated
    /// projection of native state — not the source of truth.
    fn export_markdown(plan: &Plan) -> String {
        use TaskStatus::*;

        let mut out = format!("# Plan — {}\n\n", plan.project_id);
        // Preamble: the intro prose carried from the imported source.
        if !plan.preamble.is_empty() {
            out.push_str(&plan.preamble);
            out.push_str("\n\n");
        }
        let mut listed: Vec<&Task> = plan.tasks.iter().collect();
        listed.sort_by_key(|c| c.priority);

        // (heading, statuses that fall under it), in display order. Waiting
        // tasks render alongside ToDo so the "what's next" view stays whole.
        let sections: [(&str, &[TaskStatus]); 5] = [
            ("In progress", &[InProgress]),
            ("To do", &[ToDo, Waiting]),
            ("Done", &[Done]),
            ("Ideas", &[Idea]),
            ("Retired", &[Retired]),
        ];

        for (heading, statuses) in sections {
            let mut wrote_heading = false;
            let mut current_band: Option<&str> = None;
            for c in listed.iter().filter(|c| statuses.contains(&c.status)) {
                if !wrote_heading {
                    out.push_str(&format!("## {heading}\n\n"));
                    wrote_heading = true;
                }
                // Within a status section, break the run of tasks by band.
                // `listed` is already priority-sorted, so bands come out in
                // order and each one starts exactly once.
                let band = crate::infra::coerce::priority_band(c.priority);
                if current_band != Some(band) {
                    if current_band.is_some() {
                        out.push('\n');
                    }
                    out.push_str(&format!("### {band}\n\n"));
                    current_band = Some(band);
                }
                let icon = Self::status_icon(c.status);
                if c.description.is_empty() {
                    out.push_str(&format!("- {icon} **{}** ({})\n", c.title, c.priority));
                } else {
                    out.push_str(&format!(
                        "- {icon} **{}** ({}) — {}\n",
                        c.title, c.priority, c.description
                    ));
                }
                // The name rides as a sub-bullet beside deps and acceptance
                // rather than replacing the bold title, so the export a human
                // reads still leads with the claim and an import round-trip
                // recovers both.
                if !c.name.is_empty() {
                    out.push_str(&format!("  - name: {}\n", c.name));
                }
                if !c.deps.is_empty() {
                    out.push_str(&format!("  - deps: {}\n", c.deps.join(", ")));
                }
                // The blocker, beside deps, because a reader asking "why is
                // this not moving" is asking one question and deps answer only
                // half of it. A generated PLAN.md that omits this is a
                // document that quietly deletes its own negative results: a
                // task whose premise was DISPROVEN reads identically to one
                // nobody has got to yet.
                //
                // Both the kind and the reason, unlike the status list above,
                // which carries only the token. This view has no per-row budget
                // and no truncation, so the sentence that says why can be here
                // in full — and it is the sentence, not the token, that stops
                // the next reader re-deriving the blocker from scratch.
                if let Some(b) = &c.waiting_on {
                    out.push_str(&format!(
                        "  - blocked by: {} — {}",
                        b.kind.as_wire(),
                        b.reason
                    ));
                    if let Some(ev) = &b.evidence {
                        out.push_str(&format!(" ({ev})"));
                    }
                    out.push('\n');
                }
                for a in &c.acceptance {
                    out.push_str(&format!("  - acceptance: {a}\n"));
                }
                // The hand-written narrative, indented under the task.
                for line in c.notes.lines() {
                    out.push_str(&format!("  {line}\n"));
                }
            }
            if wrote_heading {
                out.push('\n');
            }
        }

        // Doc-level note sections (Research notes, Vision, …) preserved verbatim.
        for note in &plan.notes {
            out.push_str(&format!("## {}\n\n", note.heading));
            if !note.body.is_empty() {
                out.push_str(&note.body);
                out.push_str("\n\n");
            }
        }
        out
    }

    /// Merge an imported source into a **non-empty** plan, backfilling
    /// narrative without disturbing reconciled state: existing tasks keep their
    /// `status`/`priority` but gain `notes`/`description` (when blank) and any
    /// new `acceptance` items; new tasks are added; the preamble + note sections
    /// are filled in. Returns `(added, updated)`. This is the safe "reseed" — it
    /// captures notes for projects already imported by an older parser without
    /// reverting hand-made status changes.
    pub fn merge_from_import(
        &mut self,
        imported: crate::plan::import::ImportedPlan,
    ) -> Result<Observed<(usize, usize)>, PlanMutationError> {
        self.mutate(|edit| edit.merge_from_import(imported))
    }

    /// Seed the plan from an imported source (Phase: richer-export). Sets the
    /// `preamble` if empty, appends new note sections (dedup by heading), and
    /// adds tasks whose id is new — carrying their `notes`. Persists once.
    /// Returns the number of tasks added.
    pub fn seed_from_import(
        &mut self,
        imported: crate::plan::import::ImportedPlan,
    ) -> Result<Observed<usize>, PlanMutationError> {
        self.mutate(|edit| edit.seed_from_import(imported))
    }

    /// The most ids [`Self::get`] will answer in one call.
    ///
    /// DERIVED FROM A MEASUREMENT of this project's own board, not chosen for
    /// roundness — serializing all 570 stored tasks gives mean 2,158 B,
    /// median 1,840, p90 3,499, max 6,885. What matters is the WORST case a
    /// caller can ask for, against the ~25,000-token tool-result limit that is
    /// the reason this tool exists at all:
    ///
    /// ```text
    /// n=15   76,948 B  (~19.2k tok)
    /// n=20   97,469 B  (~24.4k tok)   <- the largest N that still fits
    /// n=25  117,487 B  (~29.4k tok)   OVER
    /// ```
    ///
    /// So 20 is the answer the measurement gives. A typical 20 is 43,173 B
    /// (~10.8k tok); the cap binds on the pathological call, which is what a
    /// cap is for. Re-measure before moving it: the number is a property of
    /// how fat the records have grown, not a preference.
    ///
    /// # Why the cap counts DISTINCT ids, and why that is not a detail
    ///
    /// The table above is the N largest *distinct* records. The first version
    /// of this code answered a repeated id twice, and a live probe against the
    /// real store showed what that costs: naming the single largest record 20
    /// times returned **137,772 B (~34.4k tok)** — 41% past the very bound this
    /// cap was derived from. The worst case was never `sum of the top N`, it
    /// was `N × max`, and picking the wrong extreme instance is how a measured
    /// check ends up guarding a number nothing enforces.
    ///
    /// Deduplicating is what makes the derivation true, so `ids` is a SET: the
    /// cap is checked against the distinct count and each record is answered
    /// once. That is also the more honest reading of the request — asking for
    /// the same record twice is asking for one record — and it is stated in the
    /// tool description rather than left for a caller to discover.
    pub const GET_ID_CAP: usize = 20;

    /// Fetch the FULL stored record for a named set of task ids, optionally
    /// projected to a subset of fields.
    ///
    /// The gap this fills: `status()` returns every task as one row with the
    /// title cut at `STATUS_TITLE_LEN` (100 chars) and no description, content
    /// or acceptance, while `export()` returns everything — 1.5 MB on this board,
    /// too large to be a tool result at all. Reading three records had no
    /// verb between those two.
    ///
    /// # Every refusal here is a refusal to answer silently
    ///
    /// The three ways this call can go wrong fail the same way if handled
    /// casually: they return a plausible response that is quietly incomplete,
    /// which is worse than an error because the caller acts on it.
    ///
    /// * More ids than the cap → an error naming both numbers. Truncating to
    ///   the first N would read to the caller as "those tasks do not exist".
    /// * An id that matches nothing → reported in `unknown`, never dropped.
    ///   Omission would make "no such task" and "that task has no summary"
    ///   the same answer.
    /// * A field outside [`Task::PROJECTABLE_FIELDS`] → an error naming the
    ///   field and the vocabulary. JSON:API, the prior art for `fields`,
    ///   explicitly permits EITHER rejecting or ignoring an unrecognised name
    ///   (its RFC says a server "MAY respond with a 400"), so this is a choice
    ///   and not an inheritance: ignoring it would hand back a response
    ///   projected to nothing but `id`, indistinguishable from a board where
    ///   every named task is genuinely bare.
    ///
    /// `id` survives every projection, as it does in JSON:API — a record you
    /// cannot attribute to a task is not a record.
    pub fn get(&self, ids: &[String], fields: &[String]) -> Result<serde_json::Value, String> {
        if ids.is_empty() {
            return Err(
                "ids must name at least one task — plan_status lists the ids, \
                 plan_export dumps every record"
                    .to_string(),
            );
        }
        // `ids` is a SET. Deduplicating here rather than at the end is what
        // makes `GET_ID_CAP`'s derivation true — see the constant, and the live
        // probe that caught the difference between `sum of the top N` and
        // `N × max`. First-occurrence order is kept so the response still
        // reads in the order the caller wrote.
        let mut distinct: Vec<&String> = Vec::with_capacity(ids.len());
        for id in ids {
            if !distinct.iter().any(|seen| *seen == id.as_str()) {
                distinct.push(id);
            }
        }
        if distinct.len() > Self::GET_ID_CAP {
            return Err(format!(
                "ids names {} distinct tasks, over the cap of {} — split the call \
                 rather than expecting a short answer, because a truncated response \
                 reads as 'those tasks do not exist'",
                distinct.len(),
                Self::GET_ID_CAP,
            ));
        }
        for f in fields {
            if !Task::PROJECTABLE_FIELDS.contains(&f.as_str()) {
                return Err(format!(
                    "unknown field '{f}' — plan_get projects one of: {}",
                    Task::PROJECTABLE_FIELDS.join(", "),
                ));
            }
        }

        let mut records = Vec::new();
        let mut unknown = Vec::new();
        for id in distinct {
            match self.plan.tasks.iter().find(|c| c.id == *id) {
                Some(task) => records.push(Self::project(task, fields)),
                None => unknown.push(id.clone()),
            }
        }

        Ok(serde_json::json!({
            "records": records,
            "returned": records.len(),
            "unknown": unknown,
            // Echoed so a caller can see WHICH projection produced this shape.
            // An empty list means the whole record, and saying so beats leaving
            // the reader to infer it from the keys that happen to be present.
            "fields": fields,
        }))
    }

    /// Serialize one task, keeping only `fields` (plus `id`, always). An
    /// empty `fields` is the whole record — JSON:API's own default.
    fn project(task: &Task, fields: &[String]) -> serde_json::Value {
        let mut value = serde_json::to_value(task).unwrap_or(serde_json::Value::Null);
        if fields.is_empty() {
            return value;
        }
        if let Some(map) = value.as_object_mut() {
            map.retain(|k, _| k == "id" || fields.iter().any(|f| f == k));
        }
        value
    }

    /// Max active tasks listed by `status()` before the rest are summarized as a
    /// count. Keeps the snapshot bounded regardless of plan size.
    const STATUS_LIST_CAP: usize = 60;
    /// How many recently-finished tasks `status()` includes for context.
    const STATUS_RECENT_DONE: usize = 8;
    /// Max title length echoed in a `status()` summary row.
    const STATUS_TITLE_LEN: usize = 100;

    /// Truncate a title to [`Self::STATUS_TITLE_LEN`] chars for the status
    /// snapshot, appending `…` when cut. A defensive cap independent of the
    /// import-side [`crate::plan::import::headline`] split, so even
    /// hand-entered long titles can't bloat the snapshot.
    fn truncate_title(title: &str) -> String {
        if title.chars().count() <= Self::STATUS_TITLE_LEN {
            return title.to_string();
        }
        let cut: String = title.chars().take(Self::STATUS_TITLE_LEN).collect();
        format!("{}…", cut.trim_end())
    }

    /// Checkbox-style status glyph for the markdown projection.
    fn status_icon(status: TaskStatus) -> &'static str {
        match status {
            TaskStatus::Done => "[x]",
            TaskStatus::InProgress => "[~]",
            TaskStatus::Waiting => "[!]",
            TaskStatus::Retired => "[-]",
            TaskStatus::Idea | TaskStatus::ToDo => "[ ]",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::{Domain, Persistence, PersistenceConfig};
    use crate::plan::domain::merge_plans;
    use tempfile::TempDir;

    fn engine() -> PlanEngine {
        PlanEngine::new("proj".into())
    }

    fn add(e: &mut PlanEngine, id: &str, priority: u32, deps: Vec<String>) {
        e.add_task(
            id.into(),
            format!("Task {id}"),
            TaskStatus::ToDo,
            priority,
            String::new(),
            Vec::new(),
            deps,
            false,
        )
        .unwrap();
    }

    /// The seam, not the diagnostic. A region named after an id prefix used to
    /// be written without complaint and objected to hours later by `doctor`,
    /// which by then could not say who had written it or why.
    #[test]
    fn set_feature_refuses_a_name_that_repeats_a_slug() {
        let mut e = engine();
        add(&mut e, "berth-allocation-races", 1, vec![]);
        add(&mut e, "berth-draft-limits", 2, vec![]);

        let err = e
            .set_feature("berth-allocation-races", Some("berth".into()))
            .expect_err("a slug is not a place");
        assert!(
            err.to_string().contains("berth"),
            "the caller must be told which: {err}"
        );
        assert!(
            e.plan.tasks.iter().all(|c| c.feature.is_none()),
            "a refused name must not be written"
        );

        // Capitalizing it is the same failure, and so is using a piece of it.
        assert!(
            e.set_feature("berth-draft-limits", Some("Berth".into()))
                .is_err()
        );
        assert!(
            e.set_feature("berth-draft-limits", Some("bert".into()))
                .is_err()
        );

        // A real place is accepted, and so is the one the code itself names.
        e.set_feature("berth-draft-limits", Some("Quayside".into()))
            .expect("a place name is fine");
        e.set_feature("berth-allocation-races", Some(region::UNPLACED.into()))
            .expect("the unplaced region must survive its own check");
    }

    /// Clearing is not naming, so it cannot fail the name check.
    #[test]
    fn set_feature_still_clears() {
        let mut e = engine();
        add(&mut e, "berth-draft-limits", 1, vec![]);
        e.set_feature("berth-draft-limits", Some("Quayside".into()))
            .unwrap();
        let cleared = e.set_feature("berth-draft-limits", None).unwrap();
        assert!(cleared.value.feature.is_none());
    }

    /// The seeder's replacement: it features and it does not name, and nothing it
    /// returns has been written to a task.
    #[test]
    fn proposing_features_places_nothing() {
        let mut e = engine();
        for id in [
            "berth-allocation-races",
            "berth-draft-limits",
            "berth-pilot-handoff",
            "crane-idle-telemetry",
        ] {
            add(&mut e, id, 1, vec![]);
        }

        let proposed = e.propose_features_from_ids(3);
        assert_eq!(proposed.len(), 1);
        assert_eq!(proposed[0].prefix, "berth");
        assert!(proposed[0].why_prefix_is_unfit.contains("berth"));
        assert!(
            e.plan.tasks.iter().all(|c| c.feature.is_none()),
            "proposing must place nothing"
        );
    }

    #[test]
    fn add_task_rejects_duplicate_id() {
        let mut e = engine();
        add(&mut e, "a", 1, vec![]);
        let err = e
            .add_task(
                "a".into(),
                "dup".into(),
                TaskStatus::ToDo,
                2,
                String::new(),
                vec![],
                vec![],
                false,
            )
            .unwrap_err();
        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    fn set_status_enforces_transition_table() {
        let mut e = engine();
        add(&mut e, "a", 1, vec![]);
        e.set_status("a", TaskStatus::InProgress).unwrap();
        e.set_status("a", TaskStatus::Done).unwrap();
        // Done -> Retired is forbidden.
        let err = e.set_status("a", TaskStatus::Retired).unwrap_err();
        assert!(err.to_string().contains("illegal transition"));
    }

    #[test]
    fn next_picks_lowest_priority_ready_pending() {
        let mut e = engine();
        add(&mut e, "low", 30, vec![]);
        add(&mut e, "high", 10, vec![]);
        add(&mut e, "mid", 20, vec![]);
        assert_eq!(e.next().unwrap().id, "high");
    }

    #[test]
    fn next_skips_task_with_unsatisfied_dep() {
        let mut e = engine();
        add(&mut e, "blocker", 1, vec![]); // pending, not done
        add(&mut e, "dependent", 5, vec!["blocker".into()]);
        // dependent has lower priority would-be pick at idx but its dep isn't Done;
        // blocker (priority 1) is the ready pick instead.
        assert_eq!(e.next().unwrap().id, "blocker");
        // Now finish the blocker; dependent becomes ready.
        e.set_status("blocker", TaskStatus::InProgress).unwrap();
        e.set_status("blocker", TaskStatus::Done).unwrap();
        assert_eq!(e.next().unwrap().id, "dependent");
    }

    #[test]
    fn unknown_dep_is_treated_as_unsatisfied() {
        let mut e = engine();
        add(&mut e, "a", 1, vec!["ghost".into()]);
        assert!(e.next().is_none());
    }

    #[test]
    fn propose_reprioritize_does_not_change_priority() {
        let mut e = engine();
        add(&mut e, "a", 50, vec![]);
        e.propose_reprioritize("a", 1, "should be first".into())
            .unwrap();
        let c = &e.plan().tasks[0];
        assert_eq!(c.priority, 50, "real priority must be untouched");
        assert_eq!(c.reprioritize.as_ref().unwrap().suggested_priority, 1);
        // The proposal is surfaced but does not reorder next().
        add(&mut e, "b", 10, vec![]);
        assert_eq!(e.next().unwrap().id, "b");
    }

    #[test]
    fn obsolete_records_reason() {
        let mut e = engine();
        add(&mut e, "a", 1, vec![]);
        e.retire_task("a", "overtaken by events".into()).unwrap();
        let c = &e.plan().tasks[0];
        assert_eq!(c.status, TaskStatus::Retired);
        assert_eq!(c.retired_reason.as_deref(), Some("overtaken by events"));
    }

    // ── proposal disposal (tracker-status-proposal-disposal) ──────────────

    /// The defect this test was written against: a human ACCEPTS a status
    /// suggestion by transitioning to it, and the proposal used to survive —
    /// `has_status_proposal` stayed true in every status view forever.
    #[test]
    fn a_transition_disposes_the_status_proposal_either_way() {
        let mut e = engine();
        add(&mut e, "a", 1, vec![]);

        // Differing transition: the human moved the field somewhere else, so
        // the suggestion is superseded.
        e.propose_status("a", TaskStatus::Done, "ticket closed".into(), "t".into())
            .unwrap();
        assert!(e.plan().tasks[0].status_proposal.is_some());
        e.set_status("a", TaskStatus::InProgress).unwrap();
        assert!(
            e.plan().tasks[0].status_proposal.is_none(),
            "a differing transition must dispose the proposal — the human \
             acted on the field, which is all a proposal ever asks for"
        );

        // And the status view stops lying (asserted while the task is still
        // active — done tasks leave the reported list entirely).
        let status = e.status();
        let seen = status["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == "a")
            .expect("an in-progress task is listed");
        assert_eq!(
            seen["has_status_proposal"], false,
            "the disposal must reach the reporting surface"
        );

        // Matching transition: accepting the suggestion IS the disposal.
        e.propose_status("a", TaskStatus::Done, "ticket closed".into(), "t".into())
            .unwrap();
        e.set_status("a", TaskStatus::Done).unwrap();
        assert!(
            e.plan().tasks[0].status_proposal.is_none(),
            "accepting a suggestion by transitioning to it must dispose it"
        );
    }

    #[test]
    fn obsoleting_disposes_the_status_proposal_too() {
        // The one status write outside set_status must obey the same rule.
        let mut e = engine();
        add(&mut e, "a", 1, vec![]);
        e.propose_status("a", TaskStatus::Done, "r".into(), "s".into())
            .unwrap();
        e.retire_task("a", "overtaken".into()).unwrap();
        assert!(e.plan().tasks[0].status_proposal.is_none());
    }

    /// Disposal is FIELD-SCOPED: a transition says nothing about the title,
    /// and a contested title may be actively honored by the projector.
    #[test]
    fn a_transition_leaves_the_title_proposal_alone() {
        let mut e = engine();
        add(&mut e, "a", 1, vec![]);
        e.propose_title("a", "Their name".into(), "r".into(), "s".into())
            .unwrap();
        e.set_status("a", TaskStatus::InProgress).unwrap();
        assert!(
            e.plan().tasks[0].title_proposal.is_some(),
            "a status act must not dispose a TITLE proposal — clearing it \
             would silently drop a concession the projector is honoring"
        );
    }

    /// The reprioritize sibling of the update_task title-edit rule: an
    /// explicit differing priority edit disposes the proposal (setting the
    /// suggested value is exactly how a human accepts it); an edit that
    /// touches other fields, or restates the current priority, does not.
    #[test]
    fn a_differing_priority_edit_disposes_a_reprioritize_proposal() {
        let mut e = engine();
        add(&mut e, "a", 50, vec![]);
        e.propose_reprioritize("a", 10, "should be earlier".into())
            .unwrap();

        // A description edit is not about the priority field.
        e.update_task("a", None, None, Some("new words".into()), None, None, None)
            .unwrap();
        assert!(e.plan().tasks[0].reprioritize.is_some());

        // Restating the current priority is not a decision about the proposal.
        e.update_task("a", None, Some(50), None, None, None, None)
            .unwrap();
        assert!(e.plan().tasks[0].reprioritize.is_some());

        // Accepting it — an explicit edit to the suggested value — disposes.
        e.update_task("a", None, Some(10), None, None, None, None)
            .unwrap();
        let c = &e.plan().tasks[0];
        assert_eq!(c.priority, 10);
        assert!(
            c.reprioritize.is_none(),
            "an explicit priority edit must dispose the reprioritize proposal"
        );
    }

    /// The third sibling gains the no-restamp guard the other two always had:
    /// an unchanged suggestion must not make an old disagreement look new.
    #[test]
    fn an_unchanged_reprioritize_suggestion_does_not_restamp() {
        let mut e = engine();
        add(&mut e, "a", 50, vec![]);
        e.propose_reprioritize("a", 10, "should be earlier".into())
            .unwrap();
        let first = e.plan().tasks[0]
            .reprioritize
            .as_ref()
            .unwrap()
            .proposed_at
            .clone();
        e.propose_reprioritize("a", 10, "should be earlier".into())
            .unwrap();
        let second = e.plan().tasks[0]
            .reprioritize
            .as_ref()
            .unwrap()
            .proposed_at
            .clone();
        assert_eq!(first, second, "an unchanged suggestion was restamped");

        // A different suggestion IS a new disagreement and replaces it.
        e.propose_reprioritize("a", 5, "even earlier".into())
            .unwrap();
        assert_eq!(
            e.plan().tasks[0]
                .reprioritize
                .as_ref()
                .unwrap()
                .suggested_priority,
            5
        );
    }

    #[test]
    fn status_counts_and_next_are_reported() {
        let mut e = engine();
        add(&mut e, "a", 1, vec![]);
        add(&mut e, "b", 2, vec![]);
        e.set_status("b", TaskStatus::InProgress).unwrap();
        let s = e.status();
        assert_eq!(s["counts"]["total"], 2);
        assert_eq!(s["counts"]["to_do"], 1);
        assert_eq!(s["counts"]["in_progress"], 1);
        assert_eq!(s["next"], "a");
    }

    #[test]
    fn status_is_bounded_on_a_large_plan() {
        let mut e = engine();
        // 200 done tasks + 100 active — the shape that blew the output limit.
        for i in 0..200 {
            add(&mut e, &format!("done-{i}"), i, vec![]);
            e.set_status(&format!("done-{i}"), TaskStatus::InProgress)
                .unwrap();
            e.set_status(&format!("done-{i}"), TaskStatus::Done)
                .unwrap();
        }
        for i in 0..100 {
            add(&mut e, &format!("act-{i}"), 1000 + i, vec![]);
        }
        let s = e.status();
        // Counts still reflect the whole plan.
        assert_eq!(s["counts"]["total"], 300);
        assert_eq!(s["counts"]["done"], 200);
        // The listed tasks are active-only and capped.
        let listed = s["tasks"].as_array().unwrap();
        assert_eq!(listed.len(), PlanEngine::STATUS_LIST_CAP);
        assert!(listed.iter().all(|c| c["status"] == "to_do"));
        assert_eq!(s["active_total"], 100);
        assert_eq!(s["omitted_active"], 100 - PlanEngine::STATUS_LIST_CAP);
        assert!(s["note"].as_str().unwrap().contains("not shown"));
        // A bounded tail of finished work, never the full 200.
        assert_eq!(
            s["recent_done"].as_array().unwrap().len(),
            PlanEngine::STATUS_RECENT_DONE
        );
        // Whole snapshot stays small — the regression guard.
        let bytes = serde_json::to_string(&s).unwrap().len();
        assert!(bytes < 16_000, "status snapshot too large: {bytes} bytes");
    }

    #[test]
    fn status_truncates_long_titles() {
        let mut e = engine();
        let long = "x".repeat(500);
        e.add_task(
            "big".into(),
            long,
            TaskStatus::ToDo,
            1,
            String::new(),
            Vec::new(),
            Vec::new(),
            false,
        )
        .unwrap();
        let s = e.status();
        let title = s["tasks"][0]["title"].as_str().unwrap();
        assert!(title.chars().count() <= PlanEngine::STATUS_TITLE_LEN + 1);
        assert!(title.ends_with('…'));
    }

    #[test]
    fn persistence_round_trips_across_engine_instances() {
        let tmp = TempDir::new().unwrap();
        let cfg = PersistenceConfig::from_env()
            .with_data_dir(tmp.path().to_path_buf())
            .enabled(true);

        {
            let mut e = PlanEngine::new("proj".into())
                .with_persistence(Persistence::new(&cfg, Domain::Plan));
            add(&mut e, "a", 1, vec![]);
            e.set_status("a", TaskStatus::InProgress).unwrap();
        }

        let e2 =
            PlanEngine::new("proj".into()).with_persistence(Persistence::new(&cfg, Domain::Plan));
        assert_eq!(e2.plan().tasks.len(), 1);
        assert_eq!(e2.plan().tasks[0].status, TaskStatus::InProgress);
    }

    // ── Focus ──────────────────────────────────────────────────────────

    /// Put `id` in `feature`, bypassing `set_feature`'s region-name rules — those
    /// are that verb's business, and a focus test that had to satisfy them
    /// would be testing region naming instead of focus.
    fn grouped(e: &mut PlanEngine, id: &str, priority: u32, feature: &str) {
        add(e, id, priority, vec![]);
        e.set_feature(id, Some(feature.to_owned())).unwrap();
    }

    /// THE concurrency property, and the reason focus is a vector rather than a
    /// field: two teams hold different features at once, and neither call
    /// disturbs the other's record. A project-global focus fails this on the
    /// second `focus_set` — which is why the assertion is written against both
    /// teams AFTER both writes, not against each one as it lands.
    #[test]
    fn two_teams_focus_different_features_without_interference() {
        let mut e = engine();
        grouped(&mut e, "auth-1", 10, "Authentication");
        grouped(&mut e, "bill-1", 20, "Billing");

        e.focus_set("/work/tree-a", "Authentication", FocusMode::Build)
            .unwrap();
        e.focus_set("/work/tree-b", "Billing", FocusMode::Shape)
            .unwrap();

        let a = e.focus_get("/work/tree-a").expect("team a is focused");
        let b = e.focus_get("/work/tree-b").expect("team b is focused");
        assert_eq!(
            (a.feature.as_str(), a.mode),
            ("Authentication", FocusMode::Build)
        );
        assert_eq!((b.feature.as_str(), b.mode), ("Billing", FocusMode::Shape));
        assert_eq!(e.plan().focuses.len(), 2, "one record per team");

        // Re-focusing one team still leaves the other alone.
        e.focus_set("/work/tree-a", "Billing", FocusMode::Listen)
            .unwrap();
        assert_eq!(e.focus_get("/work/tree-b").unwrap().feature, "Billing");
        assert_eq!(e.focus_get("/work/tree-b").unwrap().mode, FocusMode::Shape);
        assert_eq!(e.plan().focuses.len(), 2, "re-focus updates, never appends");
    }

    /// THE DEFECT THIS FIXES, stated as the symptom that was actually reported:
    /// tasks created for a feature landed unassigned and were invisible to
    /// every focused read, so a focused agent saw an empty frontier and had no
    /// way to tell that from a feature with genuinely nothing ready.
    #[test]
    fn a_task_born_with_a_feature_is_immediately_visible_to_the_focused_frontier() {
        let mut e = engine();
        e.add_task_with_content(
            "auth-1".into(),
            "Rotate sessions".into(),
            String::new(),
            TaskStatus::ToDo,
            10,
            String::new(),
            vec![],
            vec![],
            false,
            None,
            Some("Authentication".into()),
        )
        .expect("created");

        // One call, not two. Before this, the task existed and the frontier
        // was empty until a separate set_feature landed.
        assert_eq!(
            e.next_in_feature("Authentication").map(|c| c.id.as_str()),
            Some("auth-1")
        );
        assert_eq!(
            e.feature_frontier(Some("Authentication"))
                .activity
                .readiness
                .ready,
            1
        );
        assert_eq!(e.features().current_names(), vec!["Authentication"]);

        // And it is reachable by focusing, which is the path that matters.
        e.focus_set("team", "Authentication", FocusMode::Build)
            .unwrap();
        assert_eq!(e.focus_get("team").unwrap().feature, "Authentication");
    }

    /// Omitting the feature still works, and blank is the same as omitted — an
    /// unassigned task is a legitimate answer, not a gap to fill.
    #[test]
    fn a_task_born_without_a_feature_is_unassigned_and_blank_means_the_same() {
        let mut e = engine();
        for (id, feature) in [("a", None), ("b", Some("   ".to_string()))] {
            e.add_task_with_content(
                id.into(),
                "t".into(),
                String::new(),
                TaskStatus::ToDo,
                10,
                String::new(),
                vec![],
                vec![],
                false,
                None,
                feature,
            )
            .expect("created");
        }
        assert!(e.plan().tasks.iter().all(|c| c.feature.is_none()));
        assert!(e.features().current_names().is_empty());
    }

    /// Both doors into grouping enforce the SAME rule, so neither is a backdoor
    /// around the other — and the birth door refuses the whole call rather than
    /// silently dropping the feature, because a task that is unassigned for an
    /// unstated reason is exactly the defect being fixed.
    #[test]
    fn an_unusable_feature_is_refused_at_birth_and_creates_nothing() {
        let mut e = engine();
        // "Billing" repeats the id prefix of `billing-1`, which names a slug
        // rather than a place. set_feature has always refused this.
        let err = e
            .add_task_with_content(
                "billing-1".into(),
                "t".into(),
                String::new(),
                TaskStatus::ToDo,
                10,
                String::new(),
                vec![],
                vec![],
                false,
                None,
                Some("Billing".into()),
            )
            .expect_err("an unusable feature must refuse");
        assert!(
            err.to_string().contains("billing"),
            "the error must name the clash: {err}"
        );
        assert!(
            e.plan().tasks.is_empty(),
            "a refused feature must not leave a task behind — least of all an unassigned one"
        );

        // The same name, via the other door, is refused identically.
        add(&mut e, "billing-1", 10, vec![]);
        let err2 = e
            .set_feature("billing-1", Some("Billing".into()))
            .expect_err("set_feature refuses it too");
        assert_eq!(
            err.to_string().trim_end_matches('.'),
            err2.to_string().trim_end_matches('.'),
            "the two doors must give the same refusal, or one is a backdoor"
        );
    }

    /// A team that has focused nothing is answered honestly rather than being
    /// handed somebody else's focus.
    #[test]
    fn an_unfocused_team_has_no_focus() {
        let mut e = engine();
        grouped(&mut e, "auth-1", 10, "Authentication");
        e.focus_set("team-a", "Authentication", FocusMode::Build)
            .unwrap();
        assert!(e.focus_get("team-b").is_none());
    }

    /// The refusal that keeps focus per-caller. A blank team must NOT become a
    /// shared default — that is the project-global bug wearing a per-team type.
    #[test]
    fn a_blank_team_is_refused_with_a_recipe_rather_than_defaulted() {
        let mut e = engine();
        grouped(&mut e, "auth-1", 10, "Authentication");

        for blank in ["", "   ", "\t"] {
            let err = e
                .focus_set(blank, "Authentication", FocusMode::Build)
                .expect_err("a blank team must be refused");
            assert!(
                err.to_string().contains("a team is required"),
                "unhelpful error: {err}"
            );
            assert!(
                err.to_string().contains("worktree") && err.to_string().contains("session"),
                "the refusal must say how to produce a real team: {err}"
            );
        }
        assert!(
            e.plan().focuses.is_empty(),
            "a refused focus must write nothing"
        );
        // And nothing sneaks in under a whitespace key either.
        assert!(e.focus_get("").is_none());
    }

    #[test]
    fn an_over_budget_or_control_character_team_is_refused() {
        let mut e = engine();
        grouped(&mut e, "auth-1", 10, "Authentication");
        let long = "x".repeat(crate::plan::domain::TEAM_BUDGET + 1);
        assert!(
            e.focus_set(&long, "Authentication", FocusMode::Build)
                .is_err()
        );
        assert!(
            e.focus_set("team\nb", "Authentication", FocusMode::Build)
                .is_err()
        );
        assert!(e.plan().focuses.is_empty());
    }

    /// An unknown feature changes nothing AND hands back the real candidates.
    #[test]
    fn an_unknown_feature_mutates_nothing_and_lists_what_exists() {
        let mut e = engine();
        grouped(&mut e, "auth-1", 10, "Authentication");
        grouped(&mut e, "bill-1", 20, "Billing");
        e.focus_set("team", "Authentication", FocusMode::Build)
            .unwrap();

        let err = e
            .focus_set("team", "Payments", FocusMode::Build)
            .expect_err("unknown feature must refuse");
        assert!(err.to_string().contains("Authentication") && err.to_string().contains("Billing"));
        assert!(err.to_string().contains("Focus is unchanged"));
        // The prior focus survived the failed switch untouched.
        let f = e.focus_get("team").unwrap();
        assert_eq!(
            (f.feature.as_str(), f.mode),
            ("Authentication", FocusMode::Build)
        );
    }

    /// An ambiguous fragment refuses too, and names exactly the candidates —
    /// picking the "closest" is how a caller ends up in a feature it never
    /// named.
    #[test]
    fn an_ambiguous_feature_fragment_mutates_nothing_and_lists_the_candidates() {
        let mut e = engine();
        grouped(&mut e, "a1", 10, "Billing core");
        grouped(&mut e, "b1", 20, "Billing reports");

        match e.features().resolve_current("billing") {
            FeatureResolution::Ambiguous { matches: names } => {
                assert_eq!(names, vec!["Billing core", "Billing reports"]);
            }
            other => panic!("expected ambiguity, got {other:?}"),
        }
        let err = e
            .focus_set("team", "billing", FocusMode::Build)
            .expect_err("ambiguous feature must refuse");
        assert!(err.to_string().contains("ambiguous"));
        assert!(e.plan().focuses.is_empty());
    }

    /// The three resolution paths, in decreasing certainty.
    #[test]
    fn a_feature_resolves_by_exact_name_by_case_and_by_unambiguous_fragment() {
        let mut e = engine();
        grouped(&mut e, "a1", 10, "Authentication and identity");
        grouped(&mut e, "b1", 20, "Billing");

        assert_eq!(
            e.features()
                .resolve_current("Authentication and identity")
                .current(),
            Some("Authentication and identity")
        );
        assert_eq!(
            e.features().resolve_current("billing").current(),
            Some("Billing"),
            "case-insensitive exact match"
        );
        assert_eq!(
            e.features().resolve_current("identity").current(),
            Some("Authentication and identity"),
            "an unambiguous fragment resolves"
        );
        // The resolved value is the STORED spelling, not what was typed.
        let f = e.focus_set("team", "identity", FocusMode::Shape).unwrap();
        assert_eq!(f.value.feature, "Authentication and identity");
    }

    /// Focusing is not starting. Nothing about any task moves.
    #[test]
    fn focusing_changes_no_task_status() {
        let mut e = engine();
        grouped(&mut e, "auth-1", 10, "Authentication");
        let before = e.plan().tasks.clone();
        e.focus_set("team", "Authentication", FocusMode::Build)
            .unwrap();
        e.focus_set("team", "Authentication", FocusMode::Listen)
            .unwrap();
        e.focus_clear("team").unwrap();
        assert_eq!(e.plan().tasks, before, "focus must not touch tasks");
    }

    /// `next_in_feature` cannot escape its feature, and says nothing rather
    /// than something from elsewhere.
    #[test]
    fn next_in_feature_never_returns_a_task_from_another_feature() {
        let mut e = engine();
        // Deliberately higher priority (lower number) OUTSIDE the feature, so a
        // naive implementation that forgot the filter would return this one.
        grouped(&mut e, "bill-1", 1, "Billing");
        grouped(&mut e, "auth-1", 500, "Authentication");
        add(&mut e, "unassigned", 2, vec![]);

        assert_eq!(e.next().map(|c| c.id.as_str()), Some("bill-1"));
        assert_eq!(
            e.next_in_feature("Authentication").map(|c| c.id.as_str()),
            Some("auth-1")
        );
        // An empty feature is an empty answer, never a fallthrough.
        assert!(e.next_in_feature("Payments").is_none());
    }

    /// A feature whose only task is blocked reports it as blocked rather than
    /// ready, and next_in_feature refuses it.
    #[test]
    fn feature_frontier_counts_readiness_blockers_and_unmet_deps() {
        let mut e = engine();
        grouped(&mut e, "one", 10, "Auth");
        grouped(&mut e, "two", 20, "Auth");
        e.plan
            .tasks
            .iter_mut()
            .find(|c| c.id == "two")
            .unwrap()
            .deps = vec!["one".into()];

        let s = e.feature_frontier(Some("Auth"));
        assert_eq!(s.activity.readiness.ready, 1, "only `one` is ready");
        assert_eq!(s.activity.next_ready.as_deref(), Some("one"));
        assert_eq!(s.activity.readiness.dependent, 1, "`two` waits on `one`");
        assert_eq!(
            s.items[1].readiness,
            crate::plan::domain::FeatureReadiness::Dependent {
                dependencies: vec![crate::plan::domain::FeatureDependency {
                    id: "one".into(),
                    status: Some(TaskStatus::ToDo),
                }],
            }
        );
        assert_eq!(s.activity.counts.total, 2);

        // Finish the dependency and the frontier moves.
        e.set_status("one", TaskStatus::InProgress).unwrap();
        e.set_status("one", TaskStatus::Done).unwrap();
        let s = e.feature_frontier(Some("Auth"));
        assert_eq!(s.activity.next_ready.as_deref(), Some("two"));
        assert_eq!(s.activity.readiness.dependent, 0);
        assert_eq!(s.activity.counts.total, 1);
        assert_eq!(e.features().history["Auth"].done, 1);
    }

    /// Focus survives a restart when persistence is on — the property that
    /// makes `switch-work` worth calling once instead of every turn.
    #[test]
    fn focus_persists_and_reloads_across_engine_instances() {
        let tmp = TempDir::new().unwrap();
        let cfg = PersistenceConfig::from_env()
            .with_data_dir(tmp.path().to_path_buf())
            .enabled(true);

        {
            let mut e = PlanEngine::new("proj".into())
                .with_persistence(Persistence::new(&cfg, Domain::Plan));
            grouped(&mut e, "auth-1", 10, "Authentication");
            grouped(&mut e, "bill-1", 20, "Billing");
            e.focus_set("team-a", "Authentication", FocusMode::Build)
                .unwrap();
            e.focus_set("team-b", "Billing", FocusMode::Listen).unwrap();
        }

        let e2 =
            PlanEngine::new("proj".into()).with_persistence(Persistence::new(&cfg, Domain::Plan));
        assert_eq!(
            e2.focus_get("team-a").map(|f| (f.feature.clone(), f.mode)),
            Some(("Authentication".to_string(), FocusMode::Build))
        );
        assert_eq!(
            e2.focus_get("team-b").map(|f| (f.feature.clone(), f.mode)),
            Some(("Billing".to_string(), FocusMode::Listen))
        );
    }

    /// The merge rule: two processes each holding their OWN team union rather
    /// than clobber, and the same team edited twice keeps the newer.
    #[test]
    fn merging_two_stores_unions_teams_and_keeps_the_newer_per_team() {
        let focus = |team: &str, feature: &str, at: &str| Focus {
            team: team.into(),
            feature: feature.into(),
            mode: FocusMode::Build,
            set_at: at.into(),
            updated_at: at.into(),
        };
        let mut memory = Plan {
            project_id: "p".into(),
            ..Default::default()
        };
        memory.focuses = vec![
            focus("team-a", "Authentication", "2026-01-02T00:00:00+00:00"),
            focus("team-shared", "Billing", "2026-01-02T00:00:00+00:00"),
        ];
        let mut disk = Plan {
            project_id: "p".into(),
            ..Default::default()
        };
        disk.focuses = vec![
            focus("team-b", "Billing", "2026-01-01T00:00:00+00:00"),
            // Older copy of a team memory also holds: memory must win.
            focus("team-shared", "Payments", "2026-01-01T00:00:00+00:00"),
        ];

        let merged = merge_plans(&memory, disk);
        let by_team = |team: &str| {
            merged
                .focuses
                .iter()
                .find(|f| f.team == team)
                .map(|f| f.feature.clone())
        };
        assert_eq!(merged.focuses.len(), 3, "disk-only teams survive the union");
        assert_eq!(by_team("team-a").as_deref(), Some("Authentication"));
        assert_eq!(by_team("team-b").as_deref(), Some("Billing"));
        assert_eq!(
            by_team("team-shared").as_deref(),
            Some("Billing"),
            "the newer copy of a contested team wins"
        );
    }

    #[test]
    fn mode_parsing_is_closed_and_names_the_alternatives() {
        assert_eq!(FocusMode::from_wire("build").unwrap(), FocusMode::Build);
        assert_eq!(FocusMode::from_wire("  SHAPE ").unwrap(), FocusMode::Shape);
        assert_eq!(FocusMode::from_wire("listen").unwrap(), FocusMode::Listen);
        let err = FocusMode::from_wire("implement").unwrap_err();
        assert!(err.contains("shape") && err.contains("build") && err.contains("listen"));
        // Wire spellings round-trip through serde exactly as typed.
        for m in FocusMode::ALL {
            assert_eq!(serde_json::to_value(m).unwrap(), m.as_wire());
        }
    }

    #[test]
    fn start_task_moves_to_in_progress() {
        let mut e = engine();
        add(&mut e, "a", 1, vec![]);
        let c = e.start_task("a").unwrap();
        assert_eq!(c.value.status, TaskStatus::InProgress);
        // The agent's reference is derived from the task id.
        assert_eq!(
            crate::infra::CrossRef::PlanTask("a".into()).to_wire(),
            "task:a"
        );
    }

    #[test]
    fn link_task_validates_and_dedupes() {
        let mut e = engine();
        add(&mut e, "a", 1, vec![]);
        e.link_task("a", "decision:42").unwrap();
        e.link_task("a", "step:auth").unwrap();
        e.link_task("a", "decision:42").unwrap(); // duplicate ignored
        assert_eq!(e.plan().tasks[0].refs, vec!["decision:42", "step:auth"]);
        // A malformed ref is rejected.
        let err = e.link_task("a", "not-a-ref").unwrap_err();
        assert!(err.to_string().contains("invalid cross-ref"));
    }

    #[test]
    fn complete_task_attaches_proof_of_run() {
        let mut e = engine();
        add(&mut e, "a", 1, vec![]);
        e.start_task("a").unwrap();
        let c = e.complete_task("a", Some("step:build-it")).unwrap();
        assert_eq!(c.value.status, TaskStatus::Done);
        assert!(c.value.refs.contains(&"step:build-it".to_string()));
    }

    #[test]
    fn complete_task_rejects_bad_ref_before_transition() {
        let mut e = engine();
        add(&mut e, "a", 1, vec![]);
        e.start_task("a").unwrap();
        let err = e.complete_task("a", Some("bogus")).unwrap_err();
        assert!(err.to_string().contains("invalid cross-ref"));
        // The status must NOT have advanced (validation happens first).
        assert_eq!(e.plan().tasks[0].status, TaskStatus::InProgress);
    }

    #[test]
    fn record_refresh_appends_a_note() {
        let mut e = engine();
        let n = e
            .record_refresh("revisited priorities".into(), vec![1, 2, 3])
            .unwrap()
            .value;
        assert_eq!(n, 1);
        let note = &e.plan().refreshes[0];
        assert_eq!(note.summary, "revisited priorities");
        assert_eq!(note.notes, vec![1, 2, 3]);
    }

    #[test]
    fn export_markdown_features_by_status_section() {
        let mut e = engine();
        e.add_task(
            "run-me".into(),
            "Run me".into(),
            TaskStatus::ToDo,
            1,
            "do the work".into(),
            vec!["it ships".into()],
            vec![],
            false,
        )
        .unwrap();
        e.add_task(
            "did-it".into(),
            "Did it".into(),
            TaskStatus::Done,
            0,
            String::new(),
            vec![],
            vec![],
            false,
        )
        .unwrap();

        let md = e.export("markdown");
        // Section headings present for the statuses that have tasks.
        assert!(md.contains("## To do"), "missing ToDo section: {md}");
        assert!(md.contains("## Done"), "missing Done section: {md}");
        // Empty sections are omitted.
        assert!(!md.contains("## Retired"), "empty section leaked: {md}");
        // A task with a description renders the em-dash tail; one without omits
        // it. Both carry their priority number, named by the band heading above.
        assert!(md.contains("**Run me** (1) — do the work"));
        assert!(md.contains("- [x] **Did it** (0)\n"));
        assert!(md.contains("  - acceptance: it ships"));
        // Both fixtures sit in the `critical` band (1 and 0), so each status
        // section opens with one band heading and no other.
        assert_eq!(
            md.matches("### critical").count(),
            2,
            "each status section should name its band exactly once: {md}"
        );
    }

    /// The bands are a lens over the same data: tasks appear under the band
    /// their stored priority falls in, in the same order as before, with the
    /// number still visible. A reader who only trusts numbers loses nothing.
    #[test]
    fn export_markdown_features_each_status_section_by_band() {
        let mut e = engine();
        for (id, priority) in [
            ("crit", 50),
            ("hi", 150),
            ("med", 250),
            ("lo", 350),
            ("later", 900),
        ] {
            e.add_task(
                id.into(),
                id.to_uppercase(),
                TaskStatus::ToDo,
                priority,
                String::new(),
                vec![],
                vec![],
                false,
            )
            .unwrap();
        }

        let md = e.export("markdown");
        let order: Vec<&str> = ["critical", "high", "medium", "low", "later"]
            .into_iter()
            .filter(|band| md.contains(&format!("### {band}")))
            .collect();
        assert_eq!(
            order,
            ["critical", "high", "medium", "low", "later"],
            "bands should appear in priority order: {md}"
        );

        // Each task keeps its raw number next to it.
        for (title, priority) in [("CRIT", 50), ("HI", 150), ("LATER", 900)] {
            assert!(
                md.contains(&format!("**{title}** ({priority})")),
                "task {title} should show its stored priority {priority}: {md}"
            );
        }
    }

    /// Bands are presentation only. Adding them must not touch what is stored
    /// or the order `next()` walks — the whole safety argument for this change.
    #[test]
    fn banding_changes_no_stored_priority_and_no_ordering() {
        let mut e = engine();
        for (id, priority) in [("c", 400), ("a", 100), ("b", 250)] {
            e.add_task(
                id.into(),
                id.into(),
                TaskStatus::ToDo,
                priority,
                String::new(),
                vec![],
                vec![],
                false,
            )
            .unwrap();
        }

        let stored: Vec<(String, u32)> = e
            .plan()
            .tasks
            .iter()
            .map(|c| (c.id.clone(), c.priority))
            .collect();
        assert_eq!(
            stored,
            vec![
                ("c".to_string(), 400),
                ("a".to_string(), 100),
                ("b".to_string(), 250)
            ],
            "stored priorities and insertion order are untouched"
        );

        // The projections still order by the number, not by the band name
        // (alphabetically "critical" < "high" < "low" < "medium" — a band-name
        // sort would put `low` before `medium` and be wrong).
        let status = e.status();
        let ids: Vec<&str> = status["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, ["a", "b", "c"], "priority order is preserved");
        assert_eq!(status["tasks"][0]["priority"], 100);
        assert_eq!(status["tasks"][0]["band"], "critical");
        assert_eq!(e.next().map(|c| c.id.as_str()), Some("a"));
    }

    #[test]
    fn merge_from_import_backfills_notes_without_reverting_status() {
        use crate::plan::import::{ImportedPlan, ImportedTask};

        let mut e = engine();
        // A task reconciled to Done by hand (the case re-import must not revert).
        e.add_task(
            "p1".into(),
            "Stage 1".into(),
            TaskStatus::Done,
            0,
            String::new(),
            vec![],
            vec![],
            false,
        )
        .unwrap();

        let imported = ImportedPlan {
            preamble: "the intro".into(),
            notes: vec![crate::plan::domain::NoteSection {
                heading: "Research".into(),
                body: "a finding".into(),
            }],
            tasks: vec![
                ImportedTask {
                    id: "p1".into(),
                    title: "Stage 1".into(),
                    name: String::new(),
                    status: TaskStatus::ToDo, // stale source status — must be ignored
                    priority: 0,
                    description: "desc".into(),
                    notes: "narrative".into(),
                    acceptance: vec!["criterion".into()],
                    deps: vec![],
                },
                ImportedTask {
                    id: "p2".into(),
                    title: "Stage 2".into(),
                    name: String::new(),
                    status: TaskStatus::ToDo,
                    priority: 1,
                    description: String::new(),
                    notes: String::new(),
                    acceptance: vec![],
                    deps: vec![],
                },
            ],
        };

        let (added, updated) = e.merge_from_import(imported).unwrap().value;
        assert_eq!((added, updated), (1, 1));
        let p1 = e.plan().tasks.iter().find(|c| c.id == "p1").unwrap();
        assert_eq!(p1.status, TaskStatus::Done, "status must NOT revert");
        assert_eq!(p1.notes, "narrative");
        assert_eq!(p1.description, "desc");
        assert_eq!(p1.acceptance, vec!["criterion".to_string()]);
        assert_eq!(e.plan().preamble, "the intro");
        assert_eq!(e.plan().notes.len(), 1);
        assert!(e.plan().tasks.iter().any(|c| c.id == "p2")); // new task added
    }

    // ---- Node labels (constraint C8) ----

    /// The task with this id. The engine has no single-task getter, and every
    /// assertion below is about one task's fields.
    fn got<'a>(e: &'a PlanEngine, id: &str) -> &'a Task {
        e.plan()
            .tasks
            .iter()
            .find(|c| c.id == id)
            .unwrap_or_else(|| panic!("no task '{id}'"))
    }

    /// The creation path a caller uses when it has nothing to say about the
    /// label. "Required or derived, never absent" is only true if this holds.
    #[test]
    fn a_task_added_without_a_name_still_gets_one() {
        let mut e = engine();
        e.add_task(
            "the-quota-ceiling-is-never-enforced".into(),
            "The quota ceiling is never enforced, so a runaway job bills forever".into(),
            TaskStatus::ToDo,
            1,
            String::new(),
            vec![],
            vec![],
            false,
        )
        .unwrap();
        let c = got(&e, "the-quota-ceiling-is-never-enforced");
        assert_eq!(c.name, "Quota ceiling");
        // The claim is untouched — this field is additive, not a rewrite.
        assert!(c.title.starts_with("The quota ceiling is never enforced"));
        assert!(crate::plan::name::fits(&c.name));
    }

    /// The leak this task exists to close: a sentence must not be installable
    /// as a label, or the defect reopens one careless call at a time.
    #[test]
    fn a_sentence_is_refused_as_a_name() {
        let mut e = engine();
        let err = e
            .add_task_with_content(
                "c1".into(),
                "A claim".into(),
                "Every task's label is a sentence, so nothing can be a node".into(),
                TaskStatus::ToDo,
                1,
                String::new(),
                vec![],
                vec![],
                false,
                None,
                None,
            )
            .unwrap_err();
        assert!(err.to_string().contains("over the"), "{err}");
        assert!(
            !e.plan().tasks.iter().any(|c| c.id == "c1"),
            "a rejected name creates no task"
        );
    }

    #[test]
    fn set_name_rewrites_the_label_and_leaves_the_title_alone() {
        let mut e = engine();
        e.add_task(
            "c1".into(),
            "A sentence stating the claim".into(),
            TaskStatus::ToDo,
            1,
            String::new(),
            vec![],
            vec![],
            false,
        )
        .unwrap();
        e.set_name("c1", "Quota ceiling").unwrap();
        assert_eq!(got(&e, "c1").name, "Quota ceiling");
        assert_eq!(got(&e, "c1").title, "A sentence stating the claim");

        // Empty re-seeds rather than clears: a nameless task stays unreachable.
        e.set_name("c1", "  ").unwrap();
        assert_eq!(got(&e, "c1").name, "C1");

        assert!(e.set_name("c1", &"x".repeat(80)).is_err());
    }

    /// The door itself: an id-less task cannot enter through `add_task`,
    /// which is how the two observed specimens got in. Drives the function,
    /// not the type — a blank and a whitespace id both bounce, and the store
    /// stays empty.
    #[test]
    fn add_task_rejects_an_empty_id() {
        let mut e = engine();
        for id in ["", "   "] {
            let err = e
                .add_task(
                    id.into(),
                    "A real claim arriving without an id".into(),
                    TaskStatus::ToDo,
                    1,
                    String::new(),
                    Vec::new(),
                    Vec::new(),
                    false,
                )
                .unwrap_err();
            assert!(
                err.to_string().contains("id"),
                "the refusal must name the id: {err}"
            );
        }
        assert!(e.plan().tasks.is_empty());
    }

    /// The cloud door: a pulled record with no id is dropped, not inserted.
    #[test]
    fn upsert_ignores_a_task_with_no_id() {
        let mut e = engine();
        let ghost: crate::plan::domain::Task = serde_json::from_value(serde_json::json!({
            "id": "", "title": "Pulled from a cloud copy that predates the door",
            "status": "to_do", "priority": 1,
            "created_at": "2026-06-08T00:00:00Z", "updated_at": "2026-06-08T00:00:00Z"
        }))
        .unwrap();
        assert!(
            e.upsert_task(ghost).is_err(),
            "an unowned record is refused"
        );
        assert!(e.plan().tasks.is_empty());
    }

    /// The export/import round trip. Both fields have to survive, and the name
    /// must not come back as an acceptance criterion.
    #[test]
    fn a_name_survives_export_and_reimport() {
        let mut e = engine();
        e.add_task(
            "reconciliation-statements-duplicate".into(),
            "Reconciliation statements duplicate every line after a retry".into(),
            TaskStatus::ToDo,
            1,
            "the description".into(),
            vec!["a criterion a human wrote".into()],
            vec![],
            false,
        )
        .unwrap();
        e.set_name(
            "reconciliation-statements-duplicate",
            "Statements duplicate",
        )
        .unwrap();

        let md = e.export("markdown");
        assert!(md.contains("- name: Statements duplicate"), "{md}");

        let back = crate::plan::import::markdown::parse_markdown(&md);
        let c = back
            .iter()
            .find(|c| c.title.starts_with("Reconciliation statements duplicate"))
            .expect("the task must survive the round trip");
        assert_eq!(c.name, "Statements duplicate");
        // The label is a field, not a criterion — it must not land in acceptance.
        assert!(
            !c.acceptance
                .iter()
                .any(|a| a.contains("Statements duplicate")),
            "the name leaked into acceptance: {:?}",
            c.acceptance
        );
        assert!(
            c.acceptance
                .iter()
                .any(|a| a.contains("a criterion a human wrote"))
        );
    }

    /// Import is the other door a task arrives through, and it must not create
    /// a nameless one either.
    #[test]
    fn an_imported_task_with_no_name_is_seeded_from_its_id() {
        let mut e = engine();
        let imported = crate::plan::import::ImportedPlan {
            preamble: String::new(),
            notes: vec![],
            tasks: vec![crate::plan::import::ImportedTask::new(
                "payroll-export-breaks-on-leap-day".into(),
                "Payroll export breaks on a leap day and drops the last run".into(),
                TaskStatus::ToDo,
                0,
            )],
        };
        assert_eq!(e.seed_from_import(imported).unwrap().value, 1);
        assert_eq!(
            got(&e, "payroll-export-breaks-on-leap-day").name,
            "Payroll export breaks"
        );
        assert!(e.tasks_without_usable_names().is_empty());
    }

    // ---- validate_waiting_on ----

    /// A blocker with no reason is the failure this whole feature replaces,
    /// wearing a struct instead of a title. Whitespace does not count as one.
    #[test]
    fn validate_waiting_on_refuses_a_reason_that_says_nothing() {
        let e = engine();
        for blank in ["", "   ", "\t", "\n  \n"] {
            let err = e
                .validate_waiting_on(WaitingReason::Person, blank.into(), None)
                .expect_err("a blank reason must be refused");
            assert!(
                err.contains("reason"),
                "the error must say what is missing: {err}"
            );
        }
    }

    /// Evidence goes through the SAME parser `refs` uses. Checked by
    /// driving the whole CrossRef vocabulary rather than one happy example, so
    /// this cannot pass while silently accepting only the form I had in mind.
    #[test]
    fn validate_waiting_on_accepts_every_cross_ref_form_and_normalizes_it() {
        let e = engine();
        for raw in [
            "decision:42",
            "task:some-other-chunk",
            "step:verify-blockedby",
            "check:cargo test",
            "progress:4",
            "ext:linear/THI-30",
        ] {
            let b = e
                .validate_waiting_on(
                    WaitingReason::Condition,
                    "waiting on the world".into(),
                    Some(raw.into()),
                )
                .unwrap_or_else(|e| panic!("'{raw}' is a legal cross-ref but was refused: {e}"));
            assert_eq!(
                b.evidence.as_deref(),
                Some(CrossRef::from_wire(raw).unwrap().to_wire().as_str()),
                "evidence must be stored in the parser's normalized form, not verbatim"
            );
        }
    }

    /// An evidence ref that resolves to nothing is worse than no evidence: it
    /// LOOKS like the blocker is sourced. Refuse it rather than store it.
    #[test]
    fn validate_waiting_on_refuses_evidence_that_is_not_a_cross_ref() {
        let e = engine();
        for bad in [
            "1763",
            "decision:",
            "just some prose",
            "decision:not-a-number",
        ] {
            let err = e
                .validate_waiting_on(
                    WaitingReason::Disproved,
                    "measured dead".into(),
                    Some(bad.into()),
                )
                .expect_err("an unparseable evidence ref must be refused");
            assert!(
                err.contains("evidence"),
                "the error must say which field is wrong: {err}"
            );
        }
    }

    /// Absent evidence is a first-class answer, not a gap to fill. A blocker
    /// nobody has a citation for is still worth recording.
    #[test]
    fn validate_waiting_on_allows_a_blocker_with_no_evidence() {
        let e = engine();
        let b = e
            .validate_waiting_on(
                WaitingReason::Outside,
                "  nobody holds the signing key  ".into(),
                None,
            )
            .expect("evidence is optional");
        assert_eq!(b.evidence, None);
        assert_eq!(b.kind, WaitingReason::Outside);
        assert_eq!(
            b.reason, "nobody holds the signing key",
            "the reason is trimmed on the way in"
        );
        assert!(
            !b.waiting_since.is_empty(),
            "a blocker must be timestamped, so 'how long has this been stuck' is answerable"
        );
    }

    /// The scope fence, flipped — and kept rather than replaced.
    ///
    /// `chunk-blocked-by-vocabulary` added the field, validated it, and
    /// deliberately did NOT teach the scheduler about it; this test asserted
    /// that blindness on purpose, so the day it ended would have to be a
    /// deliberate day. `roadmap-next-skips-blocked-by` is that day. Inverting
    /// the assertion in place — same test, opposite expectation — keeps the
    /// record that the blindness was once a choice, which deleting the test
    /// would have thrown away along with it.
    #[test]
    fn next_skips_a_task_carrying_a_blocker() {
        let mut e = engine();
        add(&mut e, "a", 1, vec![]);

        assert_eq!(
            e.next().map(|c| c.id.as_str()),
            Some("a"),
            "unblocked, this task is exactly what next() should answer — \
             which is what makes the assertion below about the blocker and \
             nothing else"
        );

        let b = e
            .validate_waiting_on(WaitingReason::Person, "needs a person".into(), None)
            .unwrap();
        let idx = e.plan().tasks.iter().position(|c| c.id == "a").unwrap();
        e.plan.tasks[idx].waiting_on = Some(b);

        assert_eq!(
            e.next(),
            None,
            "a blocker is now a refusal, and with nothing else on the board \
             the honest answer is nothing at all — not the blocked task"
        );
    }

    /// A plan this project has never had, and never will: `(id, priority,
    /// blocked)` for a greenhouse-controls plan.
    ///
    /// Foreign on purpose. A check that says "next skips blockers" while
    /// reading magistr's own tasks can pass on an accident of how the
    /// real plan happens to be ordered; this table is the only data these
    /// tests see, and [`engine_from`] takes it as a parameter so a second
    /// table can be driven through the same code without editing it.
    const GREENHOUSE: &[(&str, u32, bool)] = &[
        ("mist-line-pressure-drops", 40, false),
        ("dosing-pump-runs-dry", 5, true),
        ("vent-motor-stalls-in-heat", 12, true),
        ("substrate-ec-drifts-overnight", 25, false),
        ("light-recipe-ignores-dli", 60, false),
    ];

    /// Build a board from a table: every row a `ToDo`, dependency-free
    /// task, blocked or not as the row says. Dependency-free deliberately —
    /// deps are the refusal that already worked, and leaving them out of the
    /// table means nothing but the blocker can explain a skip.
    fn engine_from(table: &[(&str, u32, bool)]) -> PlanEngine {
        let mut e = engine();
        for (id, priority, blocked) in table {
            add(&mut e, id, *priority, vec![]);
            if *blocked {
                let b = e
                    .validate_waiting_on(
                        WaitingReason::Outside,
                        format!("{id} waits on somebody else"),
                        None,
                    )
                    .expect("a well-formed blocker");
                let idx = e
                    .plan()
                    .tasks
                    .iter()
                    .position(|c| &c.id == id)
                    .expect("just added");
                e.plan.tasks[idx].waiting_on = Some(b);
            }
        }
        e
    }

    /// The worst case is COMPUTED from the table, never named.
    ///
    /// Naming the sample is how this kind of check rots: the table gets a new
    /// row, the named task stops being the hard case, and the test keeps
    /// passing for a reason that has nothing to do with what it claims. So the
    /// test derives two answers — what a scheduler blind to blockers would say
    /// (the most urgent task on the board — smallest priority number) and what a seeing one must say
    /// (the most urgent UNBLOCKED task) — and asserts the premise that
    /// makes them differ before asserting anything about `next`.
    #[test]
    fn next_skips_the_hardest_case_the_table_can_produce() {
        let table = GREENHOUSE;
        let e = engine_from(table);

        let blind = table
            .iter()
            .min_by_key(|(_, priority, _)| *priority)
            .expect("a non-empty table");
        let seeing = table
            .iter()
            .filter(|(_, _, blocked)| !blocked)
            .min_by_key(|(_, priority, _)| *priority)
            .expect("a table with somewhere to go");

        // The premise, asserted so a re-ordering fails loudly instead of
        // quietly passing: unless the board's top task IS a blocked one, both
        // answers coincide and the assertion below proves nothing.
        assert!(
            blind.2,
            "the most urgent task in the table must be a blocked one, \
             or this test cannot tell a seeing scheduler from a blind one; \
             got '{}'",
            blind.0
        );
        assert_ne!(blind.0, seeing.0, "the two answers must be distinguishable");

        assert_eq!(
            e.next().map(|c| c.id.as_str()),
            Some(seeing.0),
            "next() must step over every blocker above '{}' — including '{}', \
             which outranks it",
            seeing.0,
            blind.0
        );
    }

    /// The other end of the same rule: a board with nothing takeable answers
    /// nothing, rather than falling back to the best blocked task. Derived
    /// from the same table so the two tests cannot disagree about the data.
    #[test]
    fn a_board_of_nothing_but_blockers_answers_nothing() {
        let all_blocked: Vec<(&str, u32, bool)> = GREENHOUSE
            .iter()
            .map(|(id, priority, _)| (*id, *priority, true))
            .collect();
        let e = engine_from(&all_blocked);

        assert_eq!(
            e.next(),
            None,
            "with every task blocked the answer is None — a scheduler that \
             relaxes its own rule when the board looks empty is worse than one \
             that admits there is nothing to do"
        );
    }

    /// Skipped is not hidden, which is the entire point of the field: a task
    /// `next` refuses keeps its status, its priority and its place in the
    /// status list, and says out loud that it is blocked. The alternative —
    /// demoting it out of the way — is exactly the workaround this line of
    /// work exists to make unnecessary.
    #[test]
    fn a_skipped_task_keeps_its_priority_and_its_place_in_status() {
        let e = engine_from(GREENHOUSE);
        let status = e.status();
        let listed = status["tasks"].as_array().expect("a task list");

        for (id, priority, blocked) in GREENHOUSE {
            let seen = listed
                .iter()
                .find(|c| c["id"] == *id)
                .unwrap_or_else(|| panic!("'{id}' vanished from status: {status}"));
            assert_eq!(
                seen["priority"].as_u64(),
                Some(u64::from(*priority)),
                "'{id}' must keep the priority that says how much it matters"
            );
            assert_eq!(
                seen["status"], "to_do",
                "'{id}' must not have been demoted to earn the skip"
            );
            // The row says WHY, not merely whether: `waiting_on` replaced the
            // `has_blocker` boolean this assertion used to read, because the
            // two could never legally disagree and only one of them answers
            // the reader's actual question. Absent is the unblocked answer.
            assert_eq!(
                seen.get("waiting_on").and_then(|k| k.as_str()),
                blocked.then_some(WaitingReason::Outside.as_wire()),
                "'{id}' must say why it is blocked, or a reader building their \
                 own frontier from this list cannot apply the skip"
            );
        }

        assert_eq!(
            status["counts"]["to_do"].as_u64(),
            Some(GREENHOUSE.len() as u64),
            "every task is still counted as pending: {status}"
        );
    }

    /// A night-operations plan for an observatory: `(id, priority, headline,
    /// tail, kind)`. The title a task actually wears is the headline, an
    /// em-dash and the tail; an unblocked row has an empty tail.
    ///
    /// Foreign on purpose, like [`GREENHOUSE`], and SPLIT on purpose. Splitting
    /// the title is what lets the truncation check derive which row is the hard
    /// case and assert that the blocker prose really does fall past the cut,
    /// instead of trusting a hand-counted character offset that the next edit
    /// to these strings would silently invalidate.
    ///
    /// [`WaitingReason::Outside`] is deliberately absent: a vocabulary entry with
    /// no tasks is the case that separates "reports 0" from "vanishes", and
    /// only a table missing one can tell them apart.
    #[allow(clippy::type_complexity)]
    const OBSERVATORY: &[(&str, u32, &str, &str, Option<WaitingReason>)] = &[
        (
            "seeing-monitor-and-guider-disagree",
            30,
            "The seeing monitor and the guide camera disagree about the sky on nights of high cirrus, and the scheduler trusts whichever answered last",
            "and the monitor's premise did not survive the measurement",
            Some(WaitingReason::Disproved),
        ),
        (
            "periodic-drift-the-encoders-deny",
            10,
            "Every exposure longer than four hundred seconds shows a periodic drift the mount's own encoders insist is not happening",
            "and nothing can be done until the new encoder firmware exists",
            Some(WaitingReason::Condition),
        ),
        (
            "dome-slit-lags-near-the-meridian",
            45,
            "The dome slit tracks the telescope with a lag that only matters near the meridian, where the flip already costs the most time",
            "and somebody has to stand in the dome and watch it happen",
            Some(WaitingReason::Person),
        ),
        (
            "dusk-and-dawn-flats-disagree",
            20,
            "Flat fields taken at dusk and at dawn differ by more than the calibration pipeline is willing to tolerate in one night",
            "and the decision about which night to sacrifice is a person's to make",
            Some(WaitingReason::Person),
        ),
        (
            "queue-ranks-targets-by-airmass-alone",
            55,
            "The queue scheduler ranks targets by airmass alone and will happily spend a whole night on one object that is already setting",
            "",
            None,
        ),
    ];

    /// The title a row wears: headline, em-dash, tail.
    fn observatory_title(headline: &str, tail: &str) -> String {
        if tail.is_empty() {
            headline.to_string()
        } else {
            format!("{headline} — {tail}")
        }
    }

    /// The blocker's reason, built so it CANNOT be found anywhere but the
    /// blocker record. A reason echoing the title would make every "the view
    /// shows the reason" assertion below pass on the title alone.
    fn observatory_reason(id: &str) -> String {
        format!("the night log for {id} says so, and its title does not")
    }

    /// Build a board from the table, writing blockers through the real engine
    /// verbs (`validate_waiting_on` then `set_waiting_on`) rather than poking
    /// the field, so what these tests read back is what a human typing
    /// `plan block` would have produced.
    #[allow(clippy::type_complexity)]
    fn observatory_from(table: &[(&str, u32, &str, &str, Option<WaitingReason>)]) -> PlanEngine {
        let mut e = engine();
        for (id, priority, headline, tail, kind) in table {
            e.add_task(
                (*id).into(),
                observatory_title(headline, tail),
                TaskStatus::ToDo,
                *priority,
                String::new(),
                Vec::new(),
                Vec::new(),
                false,
            )
            .expect("a well-formed task");
            if let Some(k) = kind {
                // A refutation cites the measurement that refuted it; the other
                // kinds have nothing to point at, which exercises both arms of
                // the optional evidence.
                let evidence = (*k == WaitingReason::Disproved)
                    .then(|| "check:mirror-recoating-audit".to_string());
                let b = e
                    .validate_waiting_on(*k, observatory_reason(id), evidence)
                    .expect("a well-formed blocker");
                e.set_waiting_on(id, b).expect("the task was just added");
            }
        }
        e
    }

    /// THE SHARP ONE. A title is cut at [`PlanEngine::STATUS_TITLE_LEN`], and
    /// on a real plan the blocker is stated in the title's TAIL — so the
    /// truncation destroys exactly the thing the reader needs, which is the
    /// measured symptom this task exists to fix.
    ///
    /// The hard case is COMPUTED, never named: among the blocked rows, the one
    /// whose headline is longest is the one whose blocker prose is pushed
    /// furthest past the cut. Naming it is how this rots — the table gains a
    /// row, the named task stops being the hard case, and the test keeps
    /// passing for a reason unrelated to its claim.
    #[test]
    fn the_waiting_on_survives_the_truncation_that_destroys_the_prose() {
        let table = OBSERVATORY;
        let e = observatory_from(table);
        let status = e.status();
        let listed = status["tasks"].as_array().expect("a task list");

        let worst = table
            .iter()
            .filter(|(_, _, _, _, kind)| kind.is_some())
            .max_by_key(|(_, _, headline, _, _)| headline.chars().count())
            .expect("a table with at least one blocker");
        let (id, _, headline, tail, kind) = worst;
        let kind = kind.expect("filtered on is_some");

        // The premise, asserted before anything about the output: unless the
        // headline alone already fills the budget, part of the tail survives
        // the cut and this test proves nothing about what truncation destroys.
        assert!(
            headline.chars().count() >= PlanEngine::STATUS_TITLE_LEN,
            "'{id}' must have a headline at least {} chars long, or its blocker \
             prose is not actually lost to truncation and the check is vacuous; \
             got {}",
            PlanEngine::STATUS_TITLE_LEN,
            headline.chars().count()
        );

        let row = listed
            .iter()
            .find(|c| c["id"] == *id)
            .unwrap_or_else(|| panic!("'{id}' vanished from status: {status}"));
        let shown = row["title"].as_str().expect("a title");

        // The loss, observed rather than assumed.
        assert!(
            shown.ends_with('…'),
            "'{id}' must actually have been cut, or nothing was destroyed: {shown}"
        );
        assert!(
            !shown.contains(tail),
            "the blocker prose must be GONE from the truncated title — that is \
             the failure being repaired, and if it survives this test is \
             measuring the wrong thing: {shown}"
        );

        // And the token that survives it.
        assert_eq!(
            row.get("waiting_on").and_then(|k| k.as_str()),
            Some(kind.as_wire()),
            "'{id}' loses its blocker to truncation and must still say what \
             kind it is: {row}"
        );

        // The complement, so absence is a fact about the blocker and not about
        // the title being short: an UNBLOCKED row that is also cut says nothing.
        let unblocked_and_cut = table
            .iter()
            .filter(|(_, _, headline, _, kind)| {
                kind.is_none() && headline.chars().count() >= PlanEngine::STATUS_TITLE_LEN
            })
            .map(|(id, ..)| *id)
            .collect::<Vec<_>>();
        assert!(
            !unblocked_and_cut.is_empty(),
            "the table must hold an unblocked row long enough to be truncated, \
             or the complement below is untested"
        );
        for id in unblocked_and_cut {
            let row = listed.iter().find(|c| c["id"] == id).expect("listed");
            assert!(
                row.get("waiting_on").is_none(),
                "'{id}' is not blocked and must carry no kind: {row}"
            );
        }
    }

    /// The counts, derived from the vocabulary rather than from a list of four
    /// names — so a kind added to [`WaitingReason`] and forgotten in `status()`
    /// fails here by name instead of being silently uncounted.
    #[test]
    fn every_kind_in_the_vocabulary_is_counted_and_the_status_buckets_do_not_move() {
        let table = OBSERVATORY;
        let e = observatory_from(table);
        let status = e.status();
        let tally = &status["counts"]["waiting_on"];

        let expected = |kind: WaitingReason| {
            table
                .iter()
                .filter(|(_, _, _, _, k)| *k == Some(kind))
                .count() as u64
        };

        // The premise. Two ways this check can go vacuous: if every kind has the
        // same tally, a `status()` that answered one number for all four would
        // pass; and if no kind has zero, "reports 0 rather than vanishing" is
        // never exercised.
        let mut tallies: Vec<u64> = WaitingReason::ALL.iter().map(|k| expected(*k)).collect();
        assert!(
            tallies.contains(&0),
            "the table must leave some kind unused, or a vanishing key reads \
             the same as an absent one"
        );
        tallies.sort_unstable();
        tallies.dedup();
        assert!(
            tallies.len() >= 3,
            "the kinds must have distinguishable tallies, or one number \
             answered for all of them would pass: {tallies:?}"
        );

        for kind in WaitingReason::ALL {
            assert_eq!(
                tally[kind.as_wire()].as_u64(),
                Some(expected(kind)),
                "'{}' must be counted, at 0 if nothing carries it: {tally}",
                kind.as_wire()
            );
        }
        assert_eq!(
            tally["total"].as_u64(),
            Some(table.iter().filter(|(_, _, _, _, k)| k.is_some()).count() as u64),
            "the total must agree with the breakdown: {tally}"
        );

        // Cross-cutting, not a seventh bucket: nothing was moved out of the
        // status partition to earn a place in the tally. This is the criterion
        // "counted separately from `idea` and from `waiting`" — a blocked
        // task is still pending, and `blocked` (the STATUS) is still zero.
        assert_eq!(
            status["counts"]["to_do"].as_u64(),
            Some(table.len() as u64),
            "every task is still counted as pending: {status}"
        );
        assert_eq!(
            status["counts"]["waiting"].as_u64(),
            Some(0),
            "no task was demoted to the Waiting STATUS to earn its tally: {status}"
        );
    }

    /// The tally counts the ACTIVE board, and the reason that is a real choice
    /// rather than a convenience: `complete_task` does not clear `waiting_on`,
    /// so a finished task really can still carry the blocker that once held it
    /// up. The premise is asserted, because if completion ever DID clear the
    /// field this test would pass while proving nothing about the filter.
    #[test]
    fn finishing_a_task_takes_it_out_of_the_unschedulable_count_without_clearing_its_blocker() {
        let mut e = observatory_from(OBSERVATORY);
        let before = e.status()["counts"]["waiting_on"]["total"]
            .as_u64()
            .expect("a total");

        // Derived: the blocked row the board would reach first.
        let victim = OBSERVATORY
            .iter()
            .filter(|(_, _, _, _, kind)| kind.is_some())
            .min_by_key(|(_, priority, ..)| *priority)
            .map(|(id, ..)| *id)
            .expect("a blocked row");

        let blocker = e
            .plan
            .tasks
            .iter()
            .find(|c| c.id == victim)
            .and_then(|c| c.waiting_on.clone())
            .expect("recorded blocker");
        e.clear_waiting_on(victim).expect("resolve before starting");
        e.start_task(victim).expect("ready pending work starts");
        e.set_waiting_on(victim, blocker)
            .expect("work becomes blocked after starting");
        e.complete_task(victim, None)
            .expect("in-progress completes");

        assert!(
            e.plan()
                .tasks
                .iter()
                .find(|c| c.id == victim)
                .expect("still stored")
                .waiting_on
                .is_some(),
            "the premise: completion must NOT clear the blocker, or the active \
             filter has nothing to exclude and this test is vacuous"
        );
        assert_eq!(
            e.status()["counts"]["waiting_on"]["total"].as_u64(),
            Some(before - 1),
            "a finished task is not part of what cannot be scheduled, however \
             stale the blocker it still carries"
        );
    }

    /// The markdown view is the one surface with no per-row budget, so it is the
    /// one that carries the SENTENCE. A generated PLAN.md that omits it is a
    /// document that quietly deletes its own negative results.
    #[test]
    fn the_markdown_view_carries_the_reason_the_status_list_cannot_afford() {
        let e = observatory_from(OBSERVATORY);
        let md = e.export("markdown");

        for (id, _, headline, tail, kind) in OBSERVATORY {
            let Some(kind) = kind else {
                continue;
            };
            let reason = observatory_reason(id);
            // The premise that keeps this from passing on the title: the reason
            // appears NOWHERE in what the view would print anyway.
            let title = observatory_title(headline, tail);
            assert!(
                !title.contains(&reason),
                "'{id}' — the reason must not be recoverable from the title, or \
                 the assertion below proves only that titles are printed"
            );
            assert!(
                md.contains(&format!("blocked by: {} — {reason}", kind.as_wire())),
                "'{id}' must state its blocker and why in the view a human \
                 reads: {md}"
            );
        }

        // Evidence rides along when there is any, and nothing is invented when
        // there is not.
        let cited = OBSERVATORY
            .iter()
            .filter(|(_, _, _, _, k)| *k == Some(WaitingReason::Disproved))
            .count();
        assert_eq!(
            md.matches("(check:mirror-recoating-audit)").count(),
            cited,
            "each refutation cites its measurement, and only those: {md}"
        );
        assert_eq!(
            md.matches("- blocked by:").count(),
            OBSERVATORY
                .iter()
                .filter(|(_, _, _, _, k)| k.is_some())
                .count(),
            "an unblocked task must not grow a blocker line: {md}"
        );
    }
}

#[cfg(test)]
mod tracker_state_tests {
    use super::*;

    fn engine_with(id: &str) -> PlanEngine {
        let mut e = PlanEngine::new("proj".into());
        e.add_task(
            id.into(),
            format!("Task {id}"),
            TaskStatus::ToDo,
            10,
            String::new(),
            Vec::new(),
            Vec::new(),
            false,
        )
        .expect("add task");
        e
    }

    /// Silence is the default and it has to be structural, not a policy applied
    /// somewhere else: a task nobody opted in is simply not in the projection
    /// set, so there is no code path that could decide otherwise.
    #[test]
    fn nothing_is_opted_in_by_default() {
        let e = engine_with("c1");
        assert!(e.tracker_opt_in("c1", "github").is_none());
        assert!(e.tasks_opted_in("github").is_empty());
    }

    #[test]
    fn opting_in_and_out_is_recorded_not_deleted() {
        let mut e = engine_with("c1");
        e.set_tracker_opt_in("c1", "GitHub", true).expect("opt in");
        assert_eq!(e.tasks_opted_in("github").len(), 1);

        e.set_tracker_opt_in("c1", "github", false)
            .expect("opt out");
        assert!(e.tasks_opted_in("github").is_empty());
        assert!(
            e.tracker_opt_in("c1", "github").is_some(),
            "the refusal must persist so a peer cannot re-enable it"
        );
    }

    /// The consequence of putting tracker state in the task envelope: both the
    /// disk merge and the cloud reconcile resolve tasks by strict recency, so
    /// a tracker write that left `updated_at` alone would produce an envelope
    /// the peer declines — and each machine would keep minting its own twin.
    #[test]
    fn a_tracker_write_bumps_the_tasks_stamp() {
        let mut e = engine_with("c1");
        let before = e.plan().tasks[0].updated_at.clone();

        e.record_tracker_link("c1", "github", "owner/repo#12", "hash-1", Some("1"))
            .expect("link");
        let after_link = e.plan().tasks[0].updated_at.clone();
        assert!(after_link > before, "the link write must bump the task");

        // The second projection reuses the SAME external id, so push_cross_ref
        // dedupes and cannot be what carries the stamp.
        e.record_tracker_link("c1", "github", "owner/repo#12", "hash-2", Some("2"))
            .expect("re-link");
        assert!(
            e.plan().tasks[0].updated_at > after_link,
            "a re-projection with an unchanged ref must still bump the task"
        );
    }

    #[test]
    fn relations_are_fenced_separately_from_content() {
        let mut e = engine_with("c1");
        e.record_tracker_link("c1", "github", "owner/repo#12", "hash-1", None)
            .expect("link");
        assert!(
            e.tracker_link("c1", "github")
                .expect("link")
                .our_last_relations_hash
                .is_none(),
            "no relation declared yet is distinct from an empty one"
        );

        e.record_tracker_relations("c1", "github", "rel-1")
            .expect("relations");
        let link = e.tracker_link("c1", "github").expect("link");
        assert_eq!(link.our_last_relations_hash.as_deref(), Some("rel-1"));
        // Recording relations must not disturb the content fence.
        assert_eq!(link.our_last_write_hash, "hash-1");
    }

    /// Relations are declared between items that already exist, so there is no
    /// meaningful "record relations for a task we never projected".
    #[test]
    fn relations_require_a_link_first() {
        let mut e = engine_with("c1");
        assert!(e.record_tracker_relations("c1", "github", "rel-1").is_err());
    }

    /// The cross-machine story end to end: what a peer receives in the task
    /// envelope is adopted under the same recency rule as everything else, so a
    /// second machine inherits the binding instead of creating a twin.
    #[test]
    fn adopted_state_follows_the_recency_rule() {
        let mut e = engine_with("c1");
        e.record_tracker_link("c1", "github", "local", "h", None)
            .expect("link");

        let stale = TrackerLink {
            task_id: "c1".into(),
            provider: "github".into(),
            external_id: "stale".into(),
            our_last_write_hash: "h".into(),
            last_seen_version: None,
            our_last_relations_hash: None,
            our_last_authored_hash: None,
            created_at: "2020-01-01T00:00:00+00:00".into(),
            updated_at: "2020-01-01T00:00:00+00:00".into(),
        };
        assert_eq!(e.adopt_tracker_state(vec![stale], vec![]).unwrap().value, 0);
        assert_eq!(
            e.tracker_link("c1", "github").expect("link").external_id,
            "local",
            "a stale peer copy must not clobber a fresher local binding"
        );

        let fresh = TrackerLink {
            external_id: "peer".into(),
            updated_at: "2099-01-01T00:00:00+00:00".into(),
            ..e.tracker_link("c1", "github").expect("link").clone()
        };
        assert_eq!(e.adopt_tracker_state(vec![fresh], vec![]).unwrap().value, 1);
        assert_eq!(
            e.tracker_link("c1", "github").expect("link").external_id,
            "peer"
        );
    }
}
