//! Pure request domain types — the LOCAL subset of the v1 wire envelope
//! (`contract/request-envelope.schema.json`). A stakeholder
//! **request** is a question / suggestion / concern / bug / feedback raised about the
//! project. The cloud-only envelope fields (`tenant_id`, `idempotency_key`,
//! `source`, `attribution`) are added on sync and are NOT stored
//! locally — locally the project_id already namespaces the partition.
//!
//! This module is pure: no IO, no persistence, no MCP. The engine
//! (`request::engine`) mediates mutations; the wire adapter (`request::mcp`)
//! exposes them. Mirrors the `plan::domain` shape (DIP — the engine depends
//! on this, never the reverse).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// What kind of stakeholder input a request is. Matches the envelope's
/// `RequestKind` vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RequestKind {
    Question,
    Suggestion,
    Concern,
    Bug,
    Feedback,
}

/// Lifecycle state of a request. The legal forward path is
/// `new → triaged → researched → raised → promoted`; any non-terminal state
/// may also be `dismissed`. Backward transitions (e.g. `researched → new`) are
/// rejected by [`RequestStatus::allows`], mirroring the contract's documented
/// lifecycle and the plan `TaskStatus` transition-table pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RequestStatus {
    New,
    Triaged,
    Researched,
    Raised,
    Promoted,
    Dismissed,
}

impl RequestKind {
    /// The wire spelling (snake_case), matching the serde representation.
    #[must_use]
    pub const fn as_wire(self) -> &'static str {
        match self {
            RequestKind::Question => "question",
            RequestKind::Suggestion => "suggestion",
            RequestKind::Concern => "concern",
            RequestKind::Bug => "bug",
            RequestKind::Feedback => "feedback",
        }
    }
}

impl RequestStatus {
    /// The wire spelling (snake_case), matching the serde representation.
    #[must_use]
    pub const fn as_wire(self) -> &'static str {
        match self {
            RequestStatus::New => "new",
            RequestStatus::Triaged => "triaged",
            RequestStatus::Researched => "researched",
            RequestStatus::Raised => "raised",
            RequestStatus::Promoted => "promoted",
            RequestStatus::Dismissed => "dismissed",
        }
    }

    /// Whether a transition from `self` to `to` is legal. One forward step along
    /// the pipeline, or a jump to `Dismissed` from any non-terminal state.
    /// `Promoted` and `Dismissed` are terminal.
    pub fn allows(self, to: RequestStatus) -> bool {
        use RequestStatus::*;
        match self {
            New => matches!(to, Triaged | Dismissed),
            Triaged => matches!(to, Researched | Dismissed),
            Researched => matches!(to, Raised | Dismissed),
            Raised => matches!(to, Promoted | Dismissed),
            Promoted | Dismissed => false,
        }
    }

    /// Progress rank for the merge-on-save conflict rule: because the
    /// lifecycle is forward-only, a higher rank IS the more recent state —
    /// no timestamp needed. `Promoted` outranks `Dismissed` deliberately:
    /// a promotion already spawned a plan task, and losing that marker
    /// while the task exists is worse than losing a concurrent dismissal.
    fn merge_rank(self) -> u8 {
        use RequestStatus::*;
        match self {
            New => 0,
            Triaged => 1,
            Researched => 2,
            Raised => 3,
            Dismissed => 4,
            Promoted => 5,
        }
    }
}

/// One agent enrichment record, populated by `request_research`.
/// Mirrors the envelope's `Enrichment`: the reasoning step that
/// produced it, the sources consulted, a summary, a confidence, and a stamp.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Enrichment {
    /// The `decision_*` step number that produced this enrichment, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<u32>,
    /// External references consulted (URLs, doc ids, ministr symbol ids).
    #[serde(default)]
    pub sources: Vec<String>,
    pub summary: String,
    /// Agent confidence in this enrichment, 0..1.
    pub confidence: f64,
    /// RFC 3339 timestamp.
    pub at: String,
}

/// A stakeholder request — the local subset of the v1 envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Request {
    /// Stable unique id (UUIDv4 minted on capture).
    pub id: String,
    pub kind: RequestKind,
    /// Human-facing attribution (display name / handle / email).
    pub from: String,
    pub body: String,
    /// Structured body (contract `$defs/StructuredContent`); `body` stays the
    /// plain fallback.
    // Advertised as a plain object in `outputSchema` (`schemars(with)`), and
    // this doc comment is deliberately one line: both are copied into each of
    // the seven Request-shaped outputSchemas, where the full shape cost ~3 KB
    // apiece against the wire budget. The shape's one home is the contract
    // file; writers meet it at [`crate::content::parse_optional`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<serde_json::Value>")]
    pub content: Option<crate::content::StructuredContent>,
    /// RFC 3339 creation timestamp.
    pub created: String,
    pub status: RequestStatus,
    /// Append-only agent enrichment trail.
    #[serde(default)]
    pub enrichment: Vec<Enrichment>,
    /// Typed cross-refs into the decision/run/plan graph in `prefix:value`
    /// wire form (the `request:` variant is written by the promotion path).
    #[serde(default)]
    pub refs: Vec<String>,
    /// LOCAL-only raising state: when the request was last raised
    /// to the human (RFC 3339). Set by `request_raise`; excludes the request
    /// from `request_ready` so it isn't re-raised. Not part of the cloud
    /// envelope subset — the backend ignores it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raised_at: Option<String>,
    /// LOCAL-only raising state: suppress from `request_ready`
    /// until this RFC 3339 instant passes. Set by `request_snooze`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snooze_until: Option<String>,
    /// LOCAL-only origin stamp (store-prune-decision-request): which project
    /// captured this request. Stamped at capture; `None` on every request
    /// recorded before the stamp existed, which is the unprovable-origin case
    /// and means `prune` will keep it unless an operator names it explicitly.
    ///
    /// Not part of the cloud envelope subset — the backend neither sends nor
    /// reads it, matching `raised_at` / `snooze_until`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
}

/// The persisted per-project request store — a CACHE of the cloud
/// system-of-record. Serialized whole to
/// `requests/sessions/<project_id>.json`, mirroring `plan::domain::Plan`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Requests {
    /// The saved format. A document written before the plan words has none
    /// and reads as 0, which readers refuse until its owner has upgraded it.
    #[serde(default)]
    pub schema_version: u32,
    pub project_id: String,
    #[serde(default)]
    pub requests: Vec<Request>,
}

/// The saved request format: version 4 is the first written in the request
/// words (`requests`, `raised`, `raised_at`, kind `suggestion` and
/// `request:` references); older versions are upgraded by their owner.
pub const REQUESTS_SCHEMA_VERSION: u32 = 4;

impl Default for Requests {
    fn default() -> Self {
        Self {
            schema_version: REQUESTS_SCHEMA_VERSION,
            project_id: String::new(),
            requests: Vec::new(),
        }
    }
}

/// Union `disk` into a copy of `memory` for the locked merge-on-save
/// discipline (family-stores-merge-on-save). Requests are keyed by `id`; on a
/// conflict the copy that has progressed further wins, judged by
/// (lifecycle rank, enrichment count) — both monotonic (the status table is
/// forward-only and the enrichment trail is append-only), so "further along"
/// is a sound recency proxy without any timestamp. Ties keep memory (the
/// mutation just acked by the saving process must stick). Disk-only requests
/// surviving the union is the anti-clobber property. Known caveat
/// (documented): same-rank field edits to the SAME request (e.g. two
/// concurrent `raise` stamps) keep only one side — the LWW tradeoff the
/// decision merge also accepted.
pub fn merge_request_stores(memory: &Requests, disk: Requests) -> Requests {
    let mut merged = memory.clone();
    for disk_request in disk.requests {
        match merged.requests.iter_mut().find(|s| s.id == disk_request.id) {
            Some(mem_request) => {
                if request_wins(&disk_request, mem_request) {
                    *mem_request = disk_request;
                }
            }
            None => merged.requests.push(disk_request),
        }
    }
    merged
}

/// THE one conflict rule for two copies of the same request: `incoming` wins
/// only when it has strictly progressed — judged by (lifecycle rank,
/// enrichment count), both monotonic, so "further along" IS "newer" without a
/// timestamp. Ties keep `existing`. Shared by the disk merge
/// ([`merge_request_stores`]) and the cloud reconcile
/// (`RequestEngine::upsert_request`) so a stale cloud copy can never roll back
/// a fresher local transition (reconcile-recency-guard).
#[must_use]
pub fn request_wins(incoming: &Request, existing: &Request) -> bool {
    (incoming.status.merge_rank(), incoming.enrichment.len())
        > (existing.status.merge_rank(), existing.enrichment.len())
}

#[cfg(test)]
#[path = "records.tests.rs"]
mod tests;
