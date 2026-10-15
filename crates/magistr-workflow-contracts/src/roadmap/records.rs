//! Roadmap domain types — pure data, no infra imports (DIP).
//!
//! A [`Roadmap`] is the long-horizon plan-of-plans: an ordered set of
//! [`Chunk`]s (phases / items) that sits above `ship_*` objectives and links
//! across to `think_*` reasoning steps. These types are serde-only and depend
//! on nothing in `crate::infra` so the dependency graph stays one-directional
//! (engine → domain + infra, never the reverse).

use serde::{Deserialize, Serialize};

/// What a focused caller is doing to its workstream.
///
/// Exactly three, and closed on purpose. A mode decides which BOUNDARY applies
/// — `Shape` may not touch implementation source, `Build` may not complete over
/// a red gate, `Listen` may not process a second signal — so an open vocabulary
/// would be a vocabulary of unenforceable boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FocusMode {
    /// Decide one bounded planning or research question.
    Shape,
    /// Complete one ready roadmap chunk.
    Build,
    /// Process one stakeholder signal.
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

/// What one LANE is currently working on.
///
/// A lane is a caller — a worktree, a terminal, an agent session, a CI job.
/// Focus is deliberately NOT a single project-wide slot: this server is one
/// process serving every client that resolves to the same project id, and a
/// repository that declares its identity gives all of its worktrees that same
/// id. A project-global focus would therefore let a second agent silently
/// re-point the first one's work mid-task, and the first would never learn it
/// had moved. Keying by lane makes that impossible rather than unlikely.
///
/// Focus points AT a workstream; it never owns one. `group` is the same
/// authored [`Chunk::group`] string a tracker maps to a container and the
/// canvas draws as a region — no second taxonomy is introduced, because a
/// competing one would immediately disagree with the first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Focus {
    /// The caller identity this focus belongs to. Never empty — see
    /// [`validate_lane`], which is the only way one is admitted.
    pub lane: String,
    /// The workstream: a value of [`Chunk::group`], matched exactly as stored.
    pub group: String,
    pub mode: FocusMode,
    /// When this lane first took a focus, preserved across re-focusing so
    /// "how long has this lane been here" survives a mode change.
    pub set_at: String,
    /// When the group or mode last changed. The merge tiebreak.
    pub updated_at: String,
}

/// The maximum length of a lane token.
///
/// A lane is an identifier, not a payload. The bound exists because the token
/// is caller-supplied and lands in a persisted key: without one, a caller could
/// write an unbounded string into the store on every focus call. 128 is chosen
/// to comfortably hold the two identities callers actually have — an absolute
/// worktree path and a session uuid — with room to spare.
pub const LANE_BUDGET: usize = 128;

/// Admit a caller-supplied lane token, or say exactly why not.
///
/// The refusal is the point. There is an obvious "helpful" alternative — treat
/// a missing lane as `"default"` — and it is precisely the bug this type exists
/// to prevent: every caller that forgot to identify itself would silently share
/// one focus, which is the project-global behaviour wearing a per-lane type.
/// So a blank lane is an error carrying a recipe for producing a real one.
pub fn validate_lane(lane: &str) -> Result<String, String> {
    let trimmed = lane.trim();
    if trimmed.is_empty() {
        return Err(
            "a lane is required — focus is per-caller, and collapsing callers into one \
             shared focus would let a second agent silently re-point the first. Pass the \
             stable identity you already have: the absolute path of your worktree, your \
             session id, or your task id"
                .to_string(),
        );
    }
    if trimmed.chars().count() > LANE_BUDGET {
        return Err(format!(
            "lane is {} characters, over the {LANE_BUDGET}-character budget — a lane is an \
             identifier, not a payload",
            trimmed.chars().count()
        ));
    }
    // Control characters would corrupt any listing that prints a lane, and a
    // lane is printed on every receipt.
    if trimmed.chars().any(char::is_control) {
        return Err("lane contains a control character".to_string());
    }
    Ok(trimmed.to_string())
}

/// Lifecycle state of a roadmap chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChunkStatus {
    /// Un-prioritized idea; not yet on the critical path.
    Backlog,
    /// Prioritized work; readiness also requires no blocker and completed dependencies.
    Pending,
    /// Currently being implemented (typically by a linked ship objective).
    InProgress,
    /// Has an unmet dependency or external blocker.
    Blocked,
    /// Shipped and verified.
    Done,
    /// Overtaken by events; kept for history, never worked.
    Obsoleted,
}

impl ChunkStatus {
    /// The wire spelling (snake_case), matching the serde representation —
    /// what a surface shows and what `parse_status` reads back.
    #[must_use]
    pub const fn as_wire(self) -> &'static str {
        match self {
            ChunkStatus::Backlog => "backlog",
            ChunkStatus::Pending => "pending",
            ChunkStatus::InProgress => "in_progress",
            ChunkStatus::Blocked => "blocked",
            ChunkStatus::Done => "done",
            ChunkStatus::Obsoleted => "obsoleted",
        }
    }

    /// Whether this chunk is still WORK — the one definition of "active".
    ///
    /// Written down once because two places need the same answer for the same
    /// reason: `tracker setup`'s bulk include, and the opt-in a newly-born chunk
    /// inherits. A mature roadmap carries hundreds of `Done` chunks and mirroring
    /// them would bury a tracker in finished work nobody asked to see. Two inline
    /// `matches!` arms would drift; this cannot.
    #[must_use]
    pub fn is_active(self) -> bool {
        matches!(
            self,
            ChunkStatus::Backlog
                | ChunkStatus::Pending
                | ChunkStatus::InProgress
                | ChunkStatus::Blocked
        )
    }

    /// Whether a chunk may move from `self` to `to`. Same-status is always a
    /// permitted no-op. The table is intentionally conservative so an agent
    /// can't, say, silently un-obsolete a chunk into `InProgress`.
    pub fn allows(self, to: ChunkStatus) -> bool {
        use ChunkStatus::*;
        if self == to {
            return true;
        }
        matches!(
            (self, to),
            (Backlog, Pending)
                | (Backlog, Obsoleted)
                | (Pending, InProgress)
                | (Pending, Blocked)
                | (Pending, Backlog)
                | (Pending, Obsoleted)
                | (InProgress, Done)
                | (InProgress, Blocked)
                | (InProgress, Pending)
                | (Blocked, InProgress)
                | (Blocked, Pending)
                | (Blocked, Obsoleted)
                | (Done, InProgress) // reopen
                | (Obsoleted, Backlog) // revive
                | (Obsoleted, Pending) // revive
        )
    }
}

/// A re-prioritization proposal attached to a chunk.
///
/// The human-decision boundary is encoded in the *type*: a proposal records a
/// *suggested* priority and a reason without ever changing the chunk's real
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
/// suggested status and never touches the chunk's real `status`.
///
/// Status is where automation is most tempting and most wrong. A ticket closed
/// in the tracker looks exactly like a chunk that should go done — but a close
/// means the ticket is finished, not that the acceptance criteria were met.
/// A machine that transitions the chunk silently removes the one moment a human
/// was going to look at the evidence, which is the moment the whole roadmap
/// exists to create.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusProposal {
    pub suggested_status: ChunkStatus,
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
/// re-asserts on the round after the concession, because the local chunk
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

/// Why a chunk cannot be worked, when the answer is not "another chunk".
///
/// The vocabulary is CLOSED on purpose. An open string would re-create the
/// problem this type exists to solve — the blocker would still be prose, only
/// prose in a different field — and a scheduler cannot act on prose.
///
/// The three named kinds were not invented; each was measured on this project's
/// own roadmap, where 19 of 133 active chunks stated a blocker in their title or
/// description and nothing could query it. [`Self::External`] is deliberately
/// last so the specific kinds are not diluted into a catch-all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockerKind {
    /// Measurement disproved the chunk's central assumption.
    ///
    /// Distinct from `Obsoleted`, which is too strong — a re-scoped version may
    /// still be viable — and from `Backlog`, which is too weak, because it reads
    /// as "not yet" rather than "disproven". A roadmap that records negative
    /// results is worth more than one that quietly deletes them, and this is the
    /// kind that makes the refutation survive.
    PremiseRefuted,
    /// Every dependency is `Done`, but the chunk waits on a world condition
    /// rather than on other work: "not until X exists", "only once the data is
    /// there". `deps` models what must SHIP first and can never model what must
    /// be TRUE first, which is why this cannot be expressed as a dependency.
    PremiseUnmet,
    /// The work needs a person at a keyboard — a recording, a physical
    /// measurement, a credential, a judgement call. Without this, the only way
    /// to stop an agent being handed work it structurally cannot start is to
    /// demote the chunk's status, which says nothing about why.
    AwaitingHuman,
    /// A blocker outside the project's control that no person here can clear by
    /// deciding to. The catch-all, and the one to reach for last.
    External,
}

impl BlockerKind {
    /// Every legal value, in declaration order — for error messages that name
    /// the vocabulary instead of merely rejecting a word.
    pub const ALL: [BlockerKind; 4] = [
        BlockerKind::PremiseRefuted,
        BlockerKind::PremiseUnmet,
        BlockerKind::AwaitingHuman,
        BlockerKind::External,
    ];

    /// The wire spelling (snake_case), matching the serde representation.
    #[must_use]
    pub fn as_wire(self) -> &'static str {
        match self {
            BlockerKind::PremiseRefuted => "premise_refuted",
            BlockerKind::PremiseUnmet => "premise_unmet",
            BlockerKind::AwaitingHuman => "awaiting_human",
            BlockerKind::External => "external",
        }
    }

    /// Parse a wire kind string, REFUSING anything outside the vocabulary.
    ///
    /// There is deliberately no `#[serde(other)]` fallback and no lenient arm:
    /// an unrecognised word must not quietly become [`Self::External`], because
    /// a mis-filed blocker is worse than a rejected one — it looks answered.
    /// The error names every legal value, matching how `parse_status` reports a
    /// bad chunk status.
    pub fn from_wire(s: &str) -> Result<Self, String> {
        Self::ALL
            .into_iter()
            .find(|k| k.as_wire() == s.trim())
            .ok_or_else(|| {
                let legal: Vec<&str> = Self::ALL.iter().map(|k| k.as_wire()).collect();
                format!("invalid blocker kind '{s}' (expected {})", legal.join("|"))
            })
    }
}

/// A first-class blocker on a chunk: why it cannot be worked, and what says so.
///
/// The fourth sibling of [`ReprioritizeProposal`], [`StatusProposal`] and
/// [`TitleProposal`], and shaped like them — a `reason` plus a timestamp — but
/// it is not a proposal. Those three record something a human must decide; this
/// records something already true about the world.
///
/// It sits beside `obsoleted_reason` because that field is the precedent: a
/// terminal state got a "why" and [`ChunkStatus::Blocked`] never did, even
/// though its own doc-comment has always promised "an unmet dependency **or
/// external blocker**". The second half of that sentence had nowhere to live
/// until this type.
///
/// `evidence` is a wire String, NOT a parsed `CrossRef`, for the same reason
/// `Chunk::cross_refs` is: this module is pure data with no `crate::infra`
/// imports (see the module header), and the parser lives in infra. Validation
/// is the engine's job — see `RoadmapEngine::validate_blocked_by` — which is
/// where every real writer goes anyway.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockedBy {
    pub kind: BlockerKind,
    /// Why, in a sentence. Required and non-blank: a blocker whose reason has
    /// to be re-derived from the title is precisely the failure this replaces.
    pub reason: String,
    /// What says so, as a cross-ref wire string (`think:42`, `chunk:foo`,
    /// `task:bar`) — or `None` when nothing does, which is an honest answer
    /// rather than a gap to fill with a guess.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
    /// When the blocker was recorded. Carried for the same reason every sibling
    /// carries `proposed_at`: "how long has this been stuck" is the first
    /// question anyone asks of a blocked plan, and a type that cannot answer it
    /// makes the reader go to git.
    pub blocked_at: String,
}

/// One unit of roadmap work — a phase / chunk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chunk {
    /// Stable slug, e.g. `"phase-26a"`. Unique within a roadmap.
    pub id: String,
    /// The claim, as a sentence. Kept verbatim: it is what a chunk *says*, and
    /// every surface that reads it today keeps reading it.
    pub title: String,
    /// The short label a canvas node wears — `title` is the sentence you read
    /// once the node is open, `name` is what lets you find it without reading.
    ///
    /// Seeded from the id by [`crate::roadmap::name::derive`] but NOT derived
    /// from it, for the same reason `group` is stored rather than computed
    /// (below): a computed label can never be corrected, and a better name is
    /// exactly the kind of judgement a human or the agent should be able to
    /// write and have stick.
    ///
    /// Budget is [`crate::roadmap::name::NAME_BUDGET`], which comes from
    /// constraint C8 rather than from taste. `#[serde(default)]` because every
    /// chunk persisted before this field existed has no `name` key, and a
    /// migration that cannot read the old state is not a migration — those are
    /// backfilled on load.
    #[serde(default)]
    pub name: String,
    pub status: ChunkStatus,
    /// Lower sorts earlier among `Pending` chunks.
    pub priority: u32,
    #[serde(default)]
    pub description: String,
    /// Structured body (contract `$defs/StructuredContent`): summary + facts +
    /// sections, written at the tool seam so the webapp renders the chunk as
    /// UI instead of a prose wall. `description` retains the authored prose;
    /// structured content is an optional current authoring form.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<crate::content::StructuredContent>,
    /// Free-form prose body carried verbatim from an imported source (the
    /// hand-written narrative under a phase). Preserved so `export` can
    /// regenerate a roadmap without nuking its detail.
    #[serde(default)]
    pub notes: String,
    /// Which workstream this chunk belongs to — the roadmap's own grouping, and
    /// what a tracker maps to a container (a Linear *project*).
    ///
    /// `None` means ungrouped, and that is a first-class answer rather than a
    /// gap to fill: a chunk with no natural home projects exactly as it always
    /// did, as a bare issue on the team. Inventing a group for every chunk was
    /// the alternative and it is worse — measured on this roadmap, 78 of 317
    /// chunks have a slug prefix shared with fewer than four others, so
    /// grouping by prefix alone would mint 56 near-empty containers.
    ///
    /// AUTHORED, never derived. The id is part of a chunk's identity; if the
    /// group were computed from it, renaming a chunk would silently move it
    /// between projects upstream. A stored field can be corrected by a human and
    /// stays put when the slug changes.
    ///
    /// It was once seeded from the id prefix, and that is now impossible rather
    /// than merely discouraged: the group doubles as the chunk's REGION on the
    /// tech-tree canvas, and `region::why_unfit` rejects a region name contained
    /// in a chunk id prefix — which a prefix always is, of itself. So
    /// `RoadmapEngine::set_group` refuses such a name outright, and
    /// `RoadmapEngine::propose_groups_from_ids` reports which chunks belong
    /// together without presuming to name them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default)]
    pub acceptance: Vec<String>,
    /// Ids of chunks that must be `Done` before this one is `next()`-able.
    #[serde(default)]
    pub deps: Vec<String>,
    /// The authored tier the tech-tree canvas's vertical axis renders — 1 is
    /// the foundation, higher is further along. Authored, never derived:
    /// dependency depth already owns the horizontal axis, and a tier says the
    /// thing depth cannot. `None` means the chunk stands off the axis, which
    /// is where every chunk starts. Written today by the canvas's ratified
    /// re-tier proposals; carried here so a CLI-side rewrite of the record
    /// cannot silently drop it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier: Option<u32>,
    /// Cross-references into other families as wire strings (e.g. `"think:42"`,
    /// `"task:auth"`). The typed `CrossRef::RoadmapChunk` integration layers
    /// on top of these raw strings.
    #[serde(default)]
    pub cross_refs: Vec<String>,
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
    /// Why this chunk was obsoleted (set when `status == Obsoleted`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub obsoleted_reason: Option<String>,
    /// Why this chunk cannot be worked, when the reason is not a dependency.
    ///
    /// `deps` answers "what must ship first" and is the ONLY readiness
    /// vocabulary the roadmap had; this answers "what must be true first", or
    /// "who must act first", or "what did we measure that killed this". A chunk
    /// carrying one keeps its priority, its band and its place in every listing
    /// — it is unschedulable, not hidden, and that separation is the point.
    ///
    /// ORTHOGONAL TO `status`, deliberately. A blocker is not a lifecycle state:
    /// [`ChunkStatus::Blocked`] says a chunk is stuck and this says why, so a
    /// `pending` chunk with a blocker and a `blocked` chunk with none are both
    /// meaningful and neither is a contradiction. Making it a status value
    /// instead would have collapsed the two questions into one answer, which is
    /// the mistake every issue tracker that models blockers as data avoids.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_by: Option<BlockedBy>,
    /// Which project recorded this chunk.
    ///
    /// The cloud envelope has always stamped `record.project_id` on the wire,
    /// but the local store dropped it on the way in — so a chunk that had
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

impl Chunk {
    /// Every top-level key a serialized [`Chunk`] can carry — the closed
    /// vocabulary `roadmap_get`'s `fields` projection accepts.
    ///
    /// A caller naming anything outside this list is refused, and the refusal
    /// quotes the list. That is the whole reason the vocabulary is a value
    /// rather than a shape: an error that says "unknown field" without saying
    /// which fields exist moves the guessing rather than ending it.
    ///
    /// Kept honest by `projectable_fields_match_the_serialized_record`, which
    /// serializes a chunk with EVERY optional field populated and compares the
    /// keys. So a field added above and forgotten here fails the suite instead
    /// of becoming quietly unfetchable — the failure mode a hand-maintained
    /// list otherwise has.
    pub const PROJECTABLE_FIELDS: &'static [&'static str] = &[
        "acceptance",
        "blocked_by",
        "content",
        "created_at",
        "cross_refs",
        "deps",
        "description",
        "group",
        "id",
        "name",
        "notes",
        "obsoleted_reason",
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

/// A datestamped refresh provenance entry (populated by `roadmap_record_refresh`
/// in 26c — links the think steps that motivated a mutation).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefreshNote {
    pub at: String,
    pub summary: String,
    #[serde(default)]
    pub think_steps: Vec<u32>,
}

/// A free-form document-level section that isn't work — a `## Research notes`,
/// `## Vision`, `## Design notes` block carried verbatim so `export` can
/// reproduce it. `heading` is the section title; `body` is its prose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteSection {
    pub heading: String,
    pub body: String,
}

/// A chunk's binding to its twin in an external tracker (tracker-port-seam).
///
/// This is the load-bearing record of the whole tracker program, and it is
/// small on purpose. Without it there is no way to answer the two questions
/// every two-way integration lives or dies on:
///
/// 1. *Create or patch?* — `external_id` resolves a chunk to the item it
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
/// It lives here rather than in `crate::tracker` because it is roadmap state: a
/// chunk's record of where it was projected. Keeping it here also keeps the
/// dependency one-way — `tracker` never learns that roadmaps exist, so a future
/// adapter depends on the port alone. That is why the fields are plain
/// `String`s rather than the `tracker` newtypes.
///
/// Identity is `(chunk_id, provider)`: one chunk has at most ONE twin per
/// provider. Two twins on one provider would make every subsequent write
/// ambiguous.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackerLink {
    pub chunk_id: String,
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
    /// Fingerprint of the blocking-link set we last declared for this chunk.
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
    /// for that answers yes forever on any chunk whose tracker-owned state has
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
        (self.chunk_id.as_str(), self.provider.as_str())
    }
}

/// One chunk's explicit consent to be projected into one provider.
///
/// Projection is opt-in per chunk AND per project, and silence is the default:
/// nobody's tracker fills up because they upgraded. The per-project half is
/// machine-local configuration (`crate::tracker::config`); this is the per-chunk
/// half, and it is roadmap state because it must travel — otherwise a second
/// machine would not know a chunk is in scope and would either skip it or, worse,
/// disagree.
///
/// Identity is `(chunk_id, provider)`, matching [`TrackerLink`], so opting in and
/// the resulting binding share one key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackerOptIn {
    pub chunk_id: String,
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
        (self.chunk_id.as_str(), self.provider.as_str())
    }
}

/// Which kind of upstream container a [`ContainerLink`] binds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContainerKind {
    /// A workstream's box — a Linear *project*.
    Group,
    /// The roof above the groups — a Linear *initiative*.
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
/// group slug, the configured initiative name) — the upstream name can drift
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

/// The project roadmap: an ordered set of chunks plus refresh provenance, and
/// (since the richer-export work) the surrounding hand-written narrative —
/// `preamble` (intro prose) and `notes` (doc-level note sections).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Roadmap {
    pub project_id: String,
    /// Intro prose between the document title and the first section.
    #[serde(default)]
    pub preamble: String,
    #[serde(default)]
    pub chunks: Vec<Chunk>,
    /// Doc-level prose sections (Research notes, Vision, …) preserved verbatim.
    #[serde(default)]
    pub notes: Vec<NoteSection>,
    #[serde(default)]
    pub refreshes: Vec<RefreshNote>,
    /// Bindings from chunks to their twins in external trackers. `#[serde(default)]`
    /// so a store written before this field existed loads unchanged.
    #[serde(default)]
    pub links: Vec<TrackerLink>,
    /// Per-chunk consent to be projected. Empty means nothing projects, which
    /// is the default and the point.
    #[serde(default)]
    pub tracker_opt_ins: Vec<TrackerOptIn>,
    /// Bindings from groups and the roadmap roof to their upstream containers.
    /// `#[serde(default)]` so a store written before this field existed loads
    /// unchanged.
    #[serde(default)]
    pub container_links: Vec<ContainerLink>,
    /// What each LANE is currently focused on, at most one entry per lane.
    ///
    /// A vector rather than a single field, and that is the whole concurrency
    /// design: there is no slot for "the" focus to be written into, so no
    /// caller can take another caller's. `#[serde(default)]` so every roadmap
    /// persisted before focus existed loads with an empty set — which is also
    /// the correct pre-focus meaning, "nobody has focused anything yet".
    #[serde(default)]
    pub focuses: Vec<Focus>,
}

/// Union `disk` into a copy of `memory` for the locked merge-on-save
/// discipline (family-stores-merge-on-save). Chunks are keyed by `id`; on a
/// conflict the copy with the newest `updated_at` wins whole — recency, not
/// status rank, because [`ChunkStatus`] is not a total order (`Done →
/// InProgress` reopen exists). Disk-only chunks surviving the union is the
/// anti-clobber property. Refresh notes union by `(at, summary)`, note
/// sections by heading, and the preamble keeps memory unless it's empty —
/// all append-shaped, so nothing acked is erased. Tracker links union by
/// `(chunk_id, provider)` on the same recency rule as chunks, so two processes
/// that both projected a chunk converge on the newer binding instead of
/// accumulating a second twin; tracker opt-ins union the same way, which is why
/// opting out is a recorded `enabled: false` rather than a deletion — a removal
/// would be re-added by any peer still holding the older opt-in. Known LWW caveat
/// (documented, same tradeoff as the think merge): two processes editing the
/// SAME chunk concurrently keep only the newer copy's fields.
pub fn merge_roadmaps(memory: &Roadmap, disk: Roadmap) -> Roadmap {
    let mut merged = memory.clone();
    for disk_chunk in disk.chunks {
        // An empty id is not an identity to key the union on, so the
        // anti-clobber rule does not apply: never resurrect an id-less disk
        // record. Without this, the load-time repair (`repair_missing_ids`)
        // removes or re-ids the record in memory and the very save that
        // persists the repair folds the disk ghost straight back in — and the
        // next load mints it a fresh `-2`, `-3`, … copy, forever.
        if disk_chunk.id.trim().is_empty() {
            continue;
        }
        match merged.chunks.iter_mut().find(|c| c.id == disk_chunk.id) {
            Some(mem_chunk) => {
                if chunk_wins(&disk_chunk, mem_chunk) {
                    *mem_chunk = disk_chunk;
                }
            }
            None => merged.chunks.push(disk_chunk),
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
    // Focus unions by LANE, newest wins — the same recency rule as every
    // sibling above, and here it is load-bearing rather than incidental. Two
    // processes each hold their own lane's focus and neither holds the other's,
    // so the common case is a pure union with no conflict to resolve at all.
    // A conflict can only arise when one lane is driven from two processes,
    // which is the one case where "the most recent thing that lane said" is
    // exactly the right answer.
    for disk_focus in disk.focuses {
        match merged
            .focuses
            .iter_mut()
            .find(|f| f.lane == disk_focus.lane)
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

/// THE one conflict rule for two copies of the same chunk: `incoming` wins
/// only when its `updated_at` is strictly newer. Ties and malformed stamps
/// keep `existing`. Shared by the disk merge ([`merge_roadmaps`]) and the
/// cloud reconcile (`RoadmapEngine::upsert_chunk`) so the system has exactly
/// one definition of "newer" per chunk (reconcile-recency-guard — a stale
/// cloud copy must never clobber a fresher local mutation).
#[must_use]
pub fn chunk_wins(incoming: &Chunk, existing: &Chunk) -> bool {
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
