//! Plan domain types — pure data, no infra imports (DIP).
//!
//! A [`Plan`] is the long-horizon plan-of-plans: an ordered set of
//! [`Task`]s (phases / items) that sits above `run_*` goals and links
//! across to `decision_*` reasoning steps. These types are serde-only and depend
//! on nothing in `crate::infra` so the dependency graph stays one-directional
//! (engine → domain + infra, never the reverse).

use serde::{Deserialize, Serialize};

/// What a focused caller is doing to its feature.
///
/// Exactly three, and closed on purpose. A mode decides which BOUNDARY applies
/// — `Shape` may not touch implementation source, `Build` may not complete over
/// a red check, `Listen` may not process a second request — so an open vocabulary
/// would be a vocabulary of unenforceable boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FocusMode {
    /// Decide one bounded planning or research question.
    Shape,
    /// Complete one ready plan task.
    Build,
    /// Process one stakeholder request.
    Listen,
}

impl FocusMode {
    /// The wire spelling, which is also what a person types.
    #[must_use]
    pub const fn as_wire(self) -> &'static str {
        match self {
            Self::Shape => "shape",
            Self::Build => "build",
            Self::Listen => "listen",
        }
    }

    /// Every mode, in the order they are offered to a caller who got it wrong.
    pub const ALL: [Self; 3] = [Self::Shape, Self::Build, Self::Listen];

    /// Parse a caller's mode, listing the alternatives when it is not one.
    ///
    /// Case- and whitespace-forgiving because this value is typed by a human
    /// through an agent, but NOT synonym-forgiving: "implement" is not silently
    /// read as `build`, because guessing a mode picks which boundary applies.
    pub fn from_wire(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "shape" => Ok(Self::Shape),
            "build" => Ok(Self::Build),
            "listen" => Ok(Self::Listen),
            other => Err(format!(
                "unknown mode '{other}' — expected one of: {}",
                Self::ALL
                    .iter()
                    .map(|m| m.as_wire())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        }
    }
}

/// What one TEAM is currently working on.
///
/// A team is a caller — a worktree, a terminal, an agent session, a CI job.
/// Focus is deliberately NOT a single project-wide slot: this server is one
/// process serving every client that resolves to the same project id, and a
/// repository that declares its identity gives all of its worktrees that same
/// id. A project-global focus would therefore let a second agent silently
/// re-point the first one's work mid-task, and the first would never learn it
/// had moved. Keying by team makes that impossible rather than unlikely.
///
/// Focus points AT a feature; it never owns one. `feature` is the same
/// authored [`Task::feature`] string a tracker maps to a container and the
/// canvas draws as a region — no second taxonomy is introduced, because a
/// competing one would immediately disagree with the first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Focus {
    /// The caller identity this focus belongs to. Never empty — see
    /// [`validate_team`], which is the only way one is admitted.
    pub team: String,
    /// The feature: a value of [`Task::feature`], matched exactly as stored.
    pub feature: String,
    pub mode: FocusMode,
    /// When this team first took a focus, preserved across re-focusing so
    /// "how long has this team been here" survives a mode change.
    pub set_at: String,
    /// When the feature or mode last changed. The merge tiebreak.
    pub updated_at: String,
}

/// The maximum length of a team token.
///
/// A team is an identifier, not a payload. The bound exists because the token
/// is caller-supplied and lands in a persisted key: without one, a caller could
/// write an unbounded string into the store on every focus call. 128 is chosen
/// to comfortably hold the two identities callers actually have — an absolute
/// worktree path and a session uuid — with room to spare.
pub const TEAM_BUDGET: usize = 128;

/// Admit a caller-supplied team token, or say exactly why not.
///
/// The refusal is the point. There is an obvious "helpful" alternative — treat
/// a missing team as `"default"` — and it is precisely the bug this type exists
/// to prevent: every caller that forgot to identify itself would silently share
/// one focus, which is the project-global behaviour wearing a per-team type.
/// So a blank team is an error carrying a recipe for producing a real one.
pub fn validate_team(team: &str) -> Result<String, String> {
    let trimmed = team.trim();
    if trimmed.is_empty() {
        return Err(
            "a team is required — focus is per-caller, and collapsing callers into one \
             shared focus would let a second agent silently re-point the first. Pass the \
             stable identity you already have: the absolute path of your worktree, your \
             session id, or your task id"
                .to_string(),
        );
    }
    if trimmed.chars().count() > TEAM_BUDGET {
        return Err(format!(
            "team is {} characters, over the {TEAM_BUDGET}-character budget — a team is an \
             identifier, not a payload",
            trimmed.chars().count()
        ));
    }
    // Control characters would corrupt any listing that prints a team, and a
    // team is printed on every receipt.
    if trimmed.chars().any(char::is_control) {
        return Err("team contains a control character".to_string());
    }
    Ok(trimmed.to_string())
}

/// Lifecycle state of a plan task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Un-prioritized idea; not yet on the critical path.
    Idea,
    /// Prioritized work; readiness also requires no waiting reason and completed dependencies.
    ToDo,
    /// Currently being implemented (typically by a linked run goal).
    InProgress,
    /// Cannot move until something named happens (see [`WaitingOn`]).
    Waiting,
    /// Shipped and verified.
    Done,
    /// Overtaken by events; kept for history, never worked.
    Retired,
}

impl TaskStatus {
    /// The wire spelling (snake_case), matching the serde representation —
    /// what a surface shows and what `parse_status` reads back.
    #[must_use]
    pub const fn as_wire(self) -> &'static str {
        match self {
            TaskStatus::Idea => "idea",
            TaskStatus::ToDo => "to_do",
            TaskStatus::InProgress => "in_progress",
            TaskStatus::Waiting => "waiting",
            TaskStatus::Done => "done",
            TaskStatus::Retired => "retired",
        }
    }

    /// Whether this task is still WORK — the one definition of "active".
    ///
    /// Written down once because two places need the same answer for the same
    /// reason: `tracker setup`'s bulk include, and the opt-in a newly-born task
    /// inherits. A mature plan carries hundreds of `Done` tasks and mirroring
    /// them would bury a tracker in finished work nobody asked to see. Two inline
    /// `matches!` arms would drift; this cannot.
    #[must_use]
    pub fn is_active(self) -> bool {
        matches!(
            self,
            TaskStatus::Idea | TaskStatus::ToDo | TaskStatus::InProgress | TaskStatus::Waiting
        )
    }

    /// Whether a task may move from `self` to `to`. Same-status is always a
    /// permitted no-op. The table is intentionally conservative so an agent
    /// can't, say, silently un-retire a task into `InProgress`.
    pub fn allows(self, to: TaskStatus) -> bool {
        use TaskStatus::*;
        if self == to {
            return true;
        }
        matches!(
            (self, to),
            (Idea, ToDo)
                | (Idea, Retired)
                | (ToDo, InProgress)
                | (ToDo, Waiting)
                | (ToDo, Idea)
                | (ToDo, Retired)
                | (InProgress, Done)
                | (InProgress, Waiting)
                | (InProgress, ToDo)
                | (Waiting, InProgress)
                | (Waiting, ToDo)
                | (Waiting, Retired)
                | (Done, InProgress) // reopen
                | (Retired, Idea) // revive
                | (Retired, ToDo) // revive
        )
    }
}

/// A re-prioritization proposal attached to a task.
///
/// The human-decision boundary is encoded in the *type*: a proposal records a
/// *suggested* priority and a reason without ever changing the task's real
/// `priority`. The agent surfaces it; only an explicit accept (a later phase)
/// applies it. This makes "never auto-reorder" an invariant, not prose
/// discipline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReprioritizeProposal {
    pub suggested_priority: u32,
    pub reason: String,
    pub proposed_at: String,
}

/// A suggested STATUS change awaiting a human decision.
///
/// The sibling of [`ReprioritizeProposal`], and for the same reason: the
/// human-decision boundary is encoded in the TYPE. A proposal records a
/// suggested status and never touches the task's real `status`.
///
/// Status is where automation is most tempting and most wrong. A ticket closed
/// in the tracker looks exactly like a task that should go done — but a close
/// means the ticket is finished, not that the acceptance criteria were met.
/// A machine that transitions the task silently removes the one moment a human
/// was going to look at the evidence, which is the moment the whole plan
/// exists to create.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusProposal {
    pub suggested_status: TaskStatus,
    pub reason: String,
    pub proposed_at: String,
    /// Where the suggestion came from, e.g. `ext:linear/THI-1`. A proposal a
    /// human cannot trace back to its source is one they cannot evaluate.
    pub source: String,
}

/// A contested TITLE the plan has conceded, awaiting a human decision.
///
/// The third sibling of [`ReprioritizeProposal`] and [`StatusProposal`], but
/// with one load-bearing difference: the projector CONSULTS it. While a title
/// proposal is open, every projection keeps sending the tracker's value
/// instead of re-asserting the plan's — this is the durable memory of the
/// concession that `Owner::Contested` deferral needs. Without it the plan
/// re-asserts on the round after the concession, because the local task
/// still says what it said and an unmoved remote must not win.
///
/// Resolution is a human act with both outcomes: ACCEPT adopts the tracker's
/// title into the plan; REJECT clears the proposal so the plan's title flows
/// again on the next projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TitleProposal {
    pub suggested_title: String,
    pub reason: String,
    pub proposed_at: String,
    /// Where the suggestion came from, e.g. `ext:linear/THI-1`. A proposal a
    /// human cannot trace back to its source is one they cannot evaluate.
    pub source: String,
}

/// Why a task cannot be worked, when the answer is not "another task".
///
/// The vocabulary is CLOSED on purpose. An open string would re-create the
/// problem this type exists to solve — the blocker would still be prose, only
/// prose in a different field — and a scheduler cannot act on prose.
///
/// The three named kinds were not invented; each was measured on this project's
/// own plan, where 19 of 133 active tasks stated a blocker in their title or
/// description and nothing could query it. [`Self::Outside`] is deliberately
/// last so the specific kinds are not diluted into a catch-all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaitingReason {
    /// Measurement disproved the task's central assumption.
    ///
    /// Distinct from `Retired`, which is too strong — a re-scoped version may
    /// still be viable — and from `Idea`, which is too weak, because it reads
    /// as "not yet" rather than "disproven". A plan that records negative
    /// results is worth more than one that quietly deletes them, and this is the
    /// kind that makes the refutation survive.
    Disproved,
    /// Every dependency is `Done`, but the task waits on a world condition
    /// rather than on other work: "not until X exists", "only once the data is
    /// there". `deps` models what must RUN first and can never model what must
    /// be TRUE first, which is why this cannot be expressed as a dependency.
    Condition,
    /// The work needs a person at a keyboard — a recording, a physical
    /// measurement, a credential, a judgement call. Without this, the only way
    /// to stop an agent being handed work it structurally cannot start is to
    /// demote the task's status, which says nothing about why.
    Person,
    /// Something outside the project's control that no person here can clear by
    /// deciding to. The catch-all, and the one to reach for last.
    Outside,
}

impl WaitingReason {
    /// Every legal value, in declaration order — for error messages that name
    /// the vocabulary instead of merely rejecting a word.
    pub const ALL: [WaitingReason; 4] = [
        WaitingReason::Disproved,
        WaitingReason::Condition,
        WaitingReason::Person,
        WaitingReason::Outside,
    ];

    /// The wire spelling (snake_case), matching the serde representation.
    #[must_use]
    pub fn as_wire(self) -> &'static str {
        match self {
            WaitingReason::Disproved => "disproved",
            WaitingReason::Condition => "condition",
            WaitingReason::Person => "person",
            WaitingReason::Outside => "outside",
        }
    }

    /// Parse a wire kind string, REFUSING anything outside the vocabulary.
    ///
    /// There is deliberately no `#[serde(other)]` fallback and no lenient arm:
    /// an unrecognised word must not quietly become [`Self::Outside`], because
    /// a mis-filed waiting reason is worse than a rejected one — it looks answered.
    /// The error names every legal value, matching how `parse_status` reports a
    /// bad task status.
    pub fn from_wire(s: &str) -> Result<Self, String> {
        Self::ALL
            .into_iter()
            .find(|k| k.as_wire() == s.trim())
            .ok_or_else(|| {
                let legal: Vec<&str> = Self::ALL.iter().map(|k| k.as_wire()).collect();
                format!(
                    "invalid waiting reason '{s}' (expected {})",
                    legal.join("|")
                )
            })
    }
}

/// A first-class blocker on a task: why it cannot be worked, and what says so.
///
/// The fourth sibling of [`ReprioritizeProposal`], [`StatusProposal`] and
/// [`TitleProposal`], and shaped like them — a `reason` plus a timestamp — but
/// it is not a proposal. Those three record something a human must decide; this
/// records something already true about the world.
///
/// It sits beside `retired_reason` because that field is the precedent: a
/// terminal state got a "why" and [`TaskStatus::Waiting`] never did, even
/// though its own doc-comment has always promised "an unmet dependency **or
/// external blocker**". The second half of that sentence had nowhere to live
/// until this type.
///
/// `evidence` is a wire String, NOT a parsed `CrossRef`, for the same reason
/// `Task::refs` is: this module is pure data with no `crate::infra`
/// imports (see the module header), and the parser lives in infra. Validation
/// is the engine's job — see `PlanEngine::validate_waiting_on` — which is
/// where every real writer goes anyway.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaitingOn {
    pub kind: WaitingReason,
    /// Why, in a sentence. Required and non-blank: a blocker whose reason has
    /// to be re-derived from the title is precisely the failure this replaces.
    pub reason: String,
    /// What says so, as a cross-ref wire string (`decision:42`, `task:foo`,
    /// `step:bar`) — or `None` when nothing does, which is an honest answer
    /// rather than a gap to fill with a guess.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
    /// When the blocker was recorded. Carried for the same reason every sibling
    /// carries `proposed_at`: "how long has this been stuck" is the first
    /// question anyone asks of a blocked plan, and a type that cannot answer it
    /// makes the reader go to git.
    pub waiting_since: String,
}

/// One unit of plan work — a phase / task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    /// Stable slug, e.g. `"phase-26a"`. Unique within a plan.
    pub id: String,
    /// The claim, as a sentence. Kept verbatim: it is what a task *says*, and
    /// every surface that reads it today keeps reading it.
    pub title: String,
    /// The short label a canvas node wears — `title` is the sentence you read
    /// once the node is open, `name` is what lets you find it without reading.
    ///
    /// Seeded from the id by [`crate::plan::name::derive`] but NOT derived
    /// from it, for the same reason `feature` is stored rather than computed
    /// (below): a computed label can never be corrected, and a better name is
    /// exactly the kind of judgement a human or the agent should be able to
    /// write and have stick.
    ///
    /// Budget is [`crate::plan::name::NAME_BUDGET`], which comes from
    /// constraint C8 rather than from taste. `#[serde(default)]` because every
    /// task persisted before this field existed has no `name` key, and a
    /// migration that cannot read the old state is not a migration — those are
    /// backfilled on load.
    #[serde(default)]
    pub name: String,
    pub status: TaskStatus,
    /// Lower sorts earlier among `ToDo` tasks.
    pub priority: u32,
    #[serde(default)]
    pub description: String,
    /// Structured body (contract `$defs/StructuredContent`): summary + facts +
    /// sections, written at the tool seam so the webapp renders the task as
    /// UI instead of a prose wall. `description` retains the authored prose;
    /// structured content is an optional current authoring form.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<crate::content::StructuredContent>,
    /// Free-form prose body carried verbatim from an imported source (the
    /// hand-written narrative under a phase). Preserved so `export` can
    /// regenerate a plan without nuking its detail.
    #[serde(default)]
    pub notes: String,
    /// Which feature this task belongs to — the plan's own grouping, and
    /// what a tracker maps to a container (a Linear *project*).
    ///
    /// `None` means unassigned, and that is a first-class answer rather than a
    /// gap to fill: a task with no natural home projects exactly as it always
    /// did, as a bare issue on the team. Inventing a feature for every task was
    /// the alternative and it is worse — measured on this plan, 78 of 317
    /// tasks have a slug prefix shared with fewer than four others, so
    /// grouping by prefix alone would mint 56 near-empty containers.
    ///
    /// AUTHORED, never derived. The id is part of a task's identity; if the
    /// feature were computed from it, renaming a task would silently move it
    /// between projects upstream. A stored field can be corrected by a human and
    /// stays put when the slug changes.
    ///
    /// It was once seeded from the id prefix, and that is now impossible rather
    /// than merely discouraged: the feature doubles as the task's REGION on the
    /// tech-tree canvas, and `region::why_unfit` rejects a region name contained
    /// in a task id prefix — which a prefix always is, of itself. So
    /// `PlanEngine::set_feature` refuses such a name outright, and
    /// `PlanEngine::propose_features_from_ids` reports which tasks belong
    /// together without presuming to name them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feature: Option<String>,
    #[serde(default)]
    pub acceptance: Vec<String>,
    /// Ids of tasks that must be `Done` before this one is `next()`-able.
    #[serde(default)]
    pub deps: Vec<String>,
    /// The authored tier the tech-tree canvas's vertical axis renders — 1 is
    /// the foundation, higher is further along. Authored, never derived:
    /// dependency depth already owns the horizontal axis, and a tier says the
    /// thing depth cannot. `None` means the task stands off the axis, which
    /// is where every task starts. Written today by the canvas's ratified
    /// re-tier proposals; carried here so a CLI-side rewrite of the record
    /// cannot silently drop it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier: Option<u32>,
    /// Cross-references into other families as wire strings (e.g. `"decision:42"`,
    /// `"step:auth"`). The typed `CrossRef::PlanTask` integration layers
    /// on top of these raw strings.
    #[serde(default)]
    pub refs: Vec<String>,
    /// `false` → gitignored `local/` partition; `true` → committed `sessions/`
    /// partition. The same local/shared split the trace stores use; only the
    /// shared partition travels through the git-native sync.
    #[serde(default)]
    pub shared: bool,
    /// A pending re-prioritization suggestion awaiting a human decision.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reprioritize: Option<ReprioritizeProposal>,
    /// A pending STATUS suggestion awaiting a human decision — a tracker said
    /// the work moved, and a person has to agree before the plan says so.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_proposal: Option<StatusProposal>,
    /// A conceded contested TITLE awaiting a human decision. Unlike its
    /// siblings this one is CONSULTED by the projector: while open, projections
    /// keep sending the tracker's title (tracker-contested-memory).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_proposal: Option<TitleProposal>,
    /// Why this task was retired (set when `status == Retired`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retired_reason: Option<String>,
    /// Why this task cannot be worked, when the reason is not a dependency.
    ///
    /// `deps` answers "what must run first" and is the ONLY readiness
    /// vocabulary the plan had; this answers "what must be true first", or
    /// "who must act first", or "what did we measure that killed this". A task
    /// carrying one keeps its priority, its band and its place in every listing
    /// — it is unschedulable, not hidden, and that separation is the point.
    ///
    /// ORTHOGONAL TO `status`, deliberately. A blocker is not a lifecycle state:
    /// [`TaskStatus::Waiting`] says a task is stuck and this says why, so a
    /// `pending` task with a blocker and a `blocked` task with none are both
    /// meaningful and neither is a contradiction. Making it a status value
    /// instead would have collapsed the two questions into one answer, which is
    /// the mistake every issue tracker that models blockers as data avoids.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waiting_on: Option<WaitingOn>,
    /// Which project recorded this task.
    ///
    /// The cloud envelope has always stamped `record.project_id` on the wire,
    /// but the local store dropped it on the way in — so a task that had
    /// arrived from another project's workspace was indistinguishable from
    /// one of ours, and the only local attribution was the file path. That is
    /// why cleaning up the 2026-06 cross-project bleed needed heuristics.
    ///
    /// `None` means "recorded before this field existed, origin unprovable".
    /// Nothing may be deleted on the strength of a `None` — see
    /// the standalone binary's retired `store_health` command.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl Task {
    /// Every top-level key a serialized [`Task`] can carry — the closed
    /// vocabulary `plan_get`'s `fields` projection accepts.
    ///
    /// A caller naming anything outside this list is refused, and the refusal
    /// quotes the list. That is the whole reason the vocabulary is a value
    /// rather than a shape: an error that says "unknown field" without saying
    /// which fields exist moves the guessing rather than ending it.
    ///
    /// Kept honest by `projectable_fields_match_the_serialized_record`, which
    /// serializes a task with EVERY optional field populated and compares the
    /// keys. So a field added above and forgotten here fails the suite instead
    /// of becoming quietly unfetchable — the failure mode a hand-maintained
    /// list otherwise has.
    pub const PROJECTABLE_FIELDS: &'static [&'static str] = &[
        "acceptance",
        "waiting_on",
        "content",
        "created_at",
        "refs",
        "deps",
        "description",
        "feature",
        "id",
        "name",
        "notes",
        "retired_reason",
        "priority",
        "project_id",
        "reprioritize",
        "shared",
        "status",
        "status_proposal",
        "tier",
        "title",
        "title_proposal",
        "updated_at",
    ];
}

/// A datestamped refresh provenance entry (populated by `plan_record_refresh`
/// in 26c — links the decision notes that motivated a mutation).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefreshNote {
    pub at: String,
    pub summary: String,
    #[serde(default)]
    pub notes: Vec<u32>,
}

/// A free-form document-level section that isn't work — a `## Research notes`,
/// `## Vision`, `## Design notes` block carried verbatim so `export` can
/// reproduce it. `heading` is the section title; `body` is its prose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteSection {
    pub heading: String,
    pub body: String,
}

/// A task's binding to its twin in an external tracker (tracker-port-seam).
///
/// This is the load-bearing record of the whole tracker program, and it is
/// small on purpose. Without it there is no way to answer the two questions
/// every two-way integration lives or dies on:
///
/// 1. *Create or patch?* — `external_id` resolves a task to the item it
///    already owns, so a replay or a restart patches instead of minting a
///    duplicate ticket.
/// 2. *Is this inbound event our own write coming back?* — `our_last_write_hash`
///    and `last_seen_version` are the fence. An event whose content matches what
///    we last wrote, at a version that has not advanced past what we recorded,
///    is an echo. Anything else is a genuine remote change. The naive
///    alternative — ignoring events attributed to our own bot user — cannot tell
///    those apart, so a human editing through our integration's token becomes
///    invisible.
///
/// It lives here rather than in `crate::tracker` because it is plan state: a
/// task's record of where it was projected. Keeping it here also keeps the
/// dependency one-way — `tracker` never learns that plans exist, so a future
/// adapter depends on the port alone. That is why the fields are plain
/// `String`s rather than the `tracker` newtypes.
///
/// Identity is `(task_id, provider)`: one task has at most ONE twin per
/// provider. Two twins on one provider would make every subsequent write
/// ambiguous.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackerLink {
    pub task_id: String,
    /// Lowercase provider key — `"github"`, `"linear"`, `"jira"`.
    pub provider: String,
    /// The id the provider minted. Case-preserving.
    pub external_id: String,
    /// `WorkItem::content_hash` of the last payload WE wrote.
    pub our_last_write_hash: String,
    /// The provider's concurrency token as of that write. `None` when the
    /// provider returns none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_seen_version: Option<String>,
    /// Fingerprint of the blocking-link set we last declared for this task.
    ///
    /// Relations live outside `our_last_write_hash` on purpose: `content_hash`
    /// covers the content we author, and a dep's external id is identity we
    /// resolved, not content. But that means a dep-only change hashes equal and
    /// the no-op short-circuit would swallow it, so relations need their own
    /// fence. `None` means no relation has ever been declared — distinct from a
    /// declared-empty set, which is how "the deps were removed" is recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub our_last_relations_hash: Option<String>,
    /// Digest of just the fields the ownership policy lets us AUTHOR, as of
    /// that same write (`tracker::ownership::authored_hash`).
    ///
    /// A second digest rather than a replacement, because the two answer
    /// different questions. `our_last_write_hash` covers everything the
    /// provider stores — including the state and labels the team owns — and
    /// must, or the echo fence would not recognize our own write coming back.
    /// This one covers only what we would ever send, which is the only honest
    /// basis for "would a projection write anything?". Comparing the full hash
    /// for that answers yes forever on any task whose tracker-owned state has
    /// moved on without us.
    ///
    /// `None` means the link predates this field. That is reported as "cannot
    /// tell without reading the tracker", never as "unchanged" and never as
    /// "would be updated" — a link written before the fence existed carries no
    /// evidence either way, and it self-heals on the next write.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub our_last_authored_hash: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl TrackerLink {
    /// The merge/lookup key.
    #[must_use]
    pub fn key(&self) -> (&str, &str) {
        (self.task_id.as_str(), self.provider.as_str())
    }
}

/// One task's explicit consent to be projected into one provider.
///
/// Projection is opt-in per task AND per project, and silence is the default:
/// nobody's tracker fills up because they upgraded. The per-project half is
/// machine-local configuration (`crate::tracker::config`); this is the per-task
/// half, and it is plan state because it must travel — otherwise a second
/// machine would not know a task is in scope and would either skip it or, worse,
/// disagree.
///
/// Identity is `(task_id, provider)`, matching [`TrackerLink`], so opting in and
/// the resulting binding share one key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackerOptIn {
    pub task_id: String,
    /// Lowercase provider key, as on [`TrackerLink`].
    pub provider: String,
    /// Opting out is recorded rather than deleted, so an explicit "stop
    /// projecting this" wins the recency merge against another machine's stale
    /// opt-in instead of being silently re-added.
    pub enabled: bool,
    pub updated_at: String,
}

impl TrackerOptIn {
    /// The merge/lookup key.
    #[must_use]
    pub fn key(&self) -> (&str, &str) {
        (self.task_id.as_str(), self.provider.as_str())
    }
}

/// Which kind of upstream container a [`ContainerLink`] binds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContainerKind {
    /// A feature's box — a Linear *project*.
    Feature,
    /// The roof above the features — a Linear *initiative*.
    Initiative,
}

/// A container's binding to its twin upstream — [`TrackerLink`]'s shape, one
/// level up (tracker-group-ownership).
///
/// This is what makes a human's RENAME survivable. Containers were resolved
/// by name alone, and a name is the one thing a human is most likely to edit:
/// the moment "tracker" became "Tracker integration", resolve-by-name missed,
/// the next push minted a duplicate project under the old name, and every
/// issue migrated into the duplicate. With the uuid remembered here, the name
/// stops being the identity — it is only what we called the thing at birth.
///
/// `created_by_us` is the fact the empty-container question turns on: a
/// future cleanup may remove a container WE minted, never one a human made.
/// It is sticky — once true, later resolves do not clear it.
///
/// Identity is `(kind, name, provider)` where `name` is OUR local name (the
/// feature slug, the configured initiative name) — the upstream name can drift
/// freely without touching the key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerLink {
    pub kind: ContainerKind,
    /// OUR name for the container — the local identity, not upstream's.
    pub name: String,
    /// Lowercase provider key, as on [`TrackerLink`].
    pub provider: String,
    /// The id the provider minted. Case-preserving.
    pub external_id: String,
    /// Whether the projector created the container, as opposed to binding to
    /// one that already existed upstream.
    pub created_by_us: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl ContainerLink {
    /// The merge/lookup key.
    #[must_use]
    pub fn key(&self) -> (ContainerKind, &str, &str) {
        (self.kind, self.name.as_str(), self.provider.as_str())
    }
}

/// The project plan: an ordered set of tasks plus refresh provenance, and
/// (since the richer-export work) the surrounding hand-written narrative —
/// `preamble` (intro prose) and `notes` (doc-level note sections).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    /// The saved format. Serialized first so admission can read it from the
    /// first bytes. A document written before the plan words has none and
    /// reads as 0, which readers refuse until its owner has upgraded it.
    #[serde(default)]
    pub schema_version: u32,
    pub project_id: String,
    /// Intro prose between the document title and the first section.
    #[serde(default)]
    pub preamble: String,
    #[serde(default)]
    pub tasks: Vec<Task>,
    /// Doc-level prose sections (Research notes, Vision, …) preserved verbatim.
    #[serde(default)]
    pub notes: Vec<NoteSection>,
    #[serde(default)]
    pub refreshes: Vec<RefreshNote>,
    /// Bindings from tasks to their twins in external trackers. `#[serde(default)]`
    /// so a store written before this field existed loads unchanged.
    #[serde(default)]
    pub links: Vec<TrackerLink>,
    /// Per-task consent to be projected. Empty means nothing projects, which
    /// is the default and the point.
    #[serde(default)]
    pub tracker_opt_ins: Vec<TrackerOptIn>,
    /// Bindings from features and the plan roof to their upstream containers.
    /// `#[serde(default)]` so a store written before this field existed loads
    /// unchanged.
    #[serde(default)]
    pub container_links: Vec<ContainerLink>,
    /// What each TEAM is currently focused on, at most one entry per team.
    ///
    /// A vector rather than a single field, and that is the whole concurrency
    /// design: there is no slot for "the" focus to be written into, so no
    /// caller can take another caller's. `#[serde(default)]` so every plan
    /// persisted before focus existed loads with an empty set — which is also
    /// the correct pre-focus meaning, "nobody has focused anything yet".
    #[serde(default)]
    pub focuses: Vec<Focus>,
}

/// The saved plan format: version 2 spells the plan words (tasks, features,
/// waiting, retired, `task:` references), 3 the decision words, 4 the request
/// words, and 5 names the team that holds each focus.
pub const PLAN_SCHEMA_VERSION: u32 = 5;

impl Default for Plan {
    fn default() -> Self {
        Self {
            schema_version: PLAN_SCHEMA_VERSION,
            project_id: String::new(),
            preamble: String::new(),
            tasks: Vec::new(),
            notes: Vec::new(),
            refreshes: Vec::new(),
            links: Vec::new(),
            tracker_opt_ins: Vec::new(),
            container_links: Vec::new(),
            focuses: Vec::new(),
        }
    }
}

/// Union `disk` into a copy of `memory` for the locked merge-on-save
/// discipline (family-stores-merge-on-save). Tasks are keyed by `id`; on a
/// conflict the copy with the newest `updated_at` wins whole — recency, not
/// status rank, because [`TaskStatus`] is not a total order (`Done →
/// InProgress` reopen exists). Disk-only tasks surviving the union is the
/// anti-clobber property. Refresh notes union by `(at, summary)`, note
/// sections by heading, and the preamble keeps memory unless it's empty —
/// all append-shaped, so nothing acked is erased. Tracker links union by
/// `(task_id, provider)` on the same recency rule as tasks, so two processes
/// that both projected a task converge on the newer binding instead of
/// accumulating a second twin; tracker opt-ins union the same way, which is why
/// opting out is a recorded `enabled: false` rather than a deletion — a removal
/// would be re-added by any peer still holding the older opt-in. Known LWW caveat
/// (documented, same tradeoff as the decision merge): two processes editing the
/// SAME task concurrently keep only the newer copy's fields.
pub fn merge_plans(memory: &Plan, disk: Plan) -> Plan {
    let mut merged = memory.clone();
    for disk_task in disk.tasks {
        // An empty id is not an identity to key the union on, so the
        // anti-clobber rule does not apply: never resurrect an id-less disk
        // record. Without this, the load-time repair (`repair_missing_ids`)
        // removes or re-ids the record in memory and the very save that
        // persists the repair folds the disk ghost straight back in — and the
        // next load mints it a fresh `-2`, `-3`, … copy, forever.
        if disk_task.id.trim().is_empty() {
            continue;
        }
        match merged.tasks.iter_mut().find(|c| c.id == disk_task.id) {
            Some(mem_task) => {
                if task_wins(&disk_task, mem_task) {
                    *mem_task = disk_task;
                }
            }
            None => merged.tasks.push(disk_task),
        }
    }
    for note in disk.notes {
        if !merged
            .notes
            .iter()
            .any(|n| n.heading.eq_ignore_ascii_case(&note.heading))
        {
            merged.notes.push(note);
        }
    }
    for refresh in disk.refreshes {
        if !merged
            .refreshes
            .iter()
            .any(|r| r.at == refresh.at && r.summary == refresh.summary)
        {
            merged.refreshes.push(refresh);
        }
    }
    for disk_link in disk.links {
        match merged.links.iter_mut().find(|l| l.key() == disk_link.key()) {
            Some(mem_link) => {
                if rfc3339_newer(&disk_link.updated_at, &mem_link.updated_at) {
                    *mem_link = disk_link;
                }
            }
            None => merged.links.push(disk_link),
        }
    }
    for disk_opt_in in disk.tracker_opt_ins {
        match merged
            .tracker_opt_ins
            .iter_mut()
            .find(|o| o.key() == disk_opt_in.key())
        {
            Some(mem_opt_in) => {
                if rfc3339_newer(&disk_opt_in.updated_at, &mem_opt_in.updated_at) {
                    *mem_opt_in = disk_opt_in;
                }
            }
            None => merged.tracker_opt_ins.push(disk_opt_in),
        }
    }
    for disk_container in disk.container_links {
        match merged
            .container_links
            .iter_mut()
            .find(|c| c.key() == disk_container.key())
        {
            Some(mem_container) => {
                // Recency picks the binding, but `created_by_us` is sticky
                // across the merge too: whichever copy remembers minting the
                // container is remembering a fact, not an opinion.
                let minted = mem_container.created_by_us || disk_container.created_by_us;
                if rfc3339_newer(&disk_container.updated_at, &mem_container.updated_at) {
                    *mem_container = disk_container;
                }
                mem_container.created_by_us = minted;
            }
            None => merged.container_links.push(disk_container),
        }
    }
    // Focus unions by TEAM, newest wins — the same recency rule as every
    // sibling above, and here it is load-bearing rather than incidental. Two
    // processes each hold their own team's focus and neither holds the other's,
    // so the common case is a pure union with no conflict to resolve at all.
    // A conflict can only arise when one team is driven from two processes,
    // which is the one case where "the most recent thing that team said" is
    // exactly the right answer.
    for disk_focus in disk.focuses {
        match merged
            .focuses
            .iter_mut()
            .find(|f| f.team == disk_focus.team)
        {
            Some(mem_focus) => {
                if rfc3339_newer(&disk_focus.updated_at, &mem_focus.updated_at) {
                    *mem_focus = disk_focus;
                }
            }
            None => merged.focuses.push(disk_focus),
        }
    }
    merged.refreshes.sort_by(|a, b| a.at.cmp(&b.at));
    if merged.preamble.is_empty() {
        merged.preamble = disk.preamble;
    }
    merged
}

/// THE one conflict rule for two copies of the same task: `incoming` wins
/// only when its `updated_at` is strictly newer. Ties and malformed stamps
/// keep `existing`. Shared by the disk merge ([`merge_plans`]) and the
/// cloud reconcile (`PlanEngine::upsert_task`) so the system has exactly
/// one definition of "newer" per task (reconcile-recency-guard — a stale
/// cloud copy must never clobber a fresher local mutation).
#[must_use]
pub fn task_wins(incoming: &Task, existing: &Task) -> bool {
    rfc3339_newer(&incoming.updated_at, &existing.updated_at)
}

/// Whether RFC 3339 stamp `a` is strictly newer than `b`. Unparseable stamps
/// compare as oldest, so a malformed timestamp can never win a merge.
#[must_use]
pub fn rfc3339_newer(a: &str, b: &str) -> bool {
    use chrono::DateTime;
    let pa = DateTime::parse_from_rfc3339(a).ok();
    let pb = DateTime::parse_from_rfc3339(b).ok();
    match (pa, pb) {
        (Some(a), Some(b)) => a > b,
        (Some(_), None) => true,
        _ => false,
    }
}

#[cfg(test)]
#[path = "records.tests.rs"]
mod tests;
