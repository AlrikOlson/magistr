//! Pure signal domain types — the LOCAL subset of the v1 wire envelope
//! (`contract/signal-envelope.schema.json`). A stakeholder
//! **signal** is a question / idea / concern / bug / feedback raised about the
//! project. The cloud-only envelope fields (`tenant_id`, `idempotency_key`,
//! `source`, `attribution`) are added on sync and are NOT stored
//! locally — locally the project_id already namespaces the partition.
//!
//! This module is pure: no IO, no persistence, no MCP. The engine
//! (`signal::engine`) mediates mutations; the wire adapter (`signal::mcp`)
//! exposes them. Mirrors the `roadmap::domain` shape (DIP — the engine depends
//! on this, never the reverse).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// What kind of stakeholder input a signal is. Matches the envelope's
/// `SignalKind` vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SignalKind {
    Question,
    Idea,
    Concern,
    Bug,
    Feedback,
}

/// Lifecycle state of a signal. The legal forward path is
/// `new → triaged → researched → surfaced → promoted`; any non-terminal state
/// may also be `dismissed`. Backward transitions (e.g. `researched → new`) are
/// rejected by [`SignalStatus::allows`], mirroring the contract's documented
/// lifecycle and the roadmap `ChunkStatus` transition-table pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SignalStatus {
    New,
    Triaged,
    Researched,
    Surfaced,
    Promoted,
    Dismissed,
}

impl SignalKind {
    /// The wire spelling (snake_case), matching the serde representation.
    #[must_use]
    pub const fn as_wire(self) -> &'static str {
        match self {
            SignalKind::Question => "question",
            SignalKind::Idea => "idea",
            SignalKind::Concern => "concern",
            SignalKind::Bug => "bug",
            SignalKind::Feedback => "feedback",
        }
    }
}

impl SignalStatus {
    /// The wire spelling (snake_case), matching the serde representation.
    #[must_use]
    pub const fn as_wire(self) -> &'static str {
        match self {
            SignalStatus::New => "new",
            SignalStatus::Triaged => "triaged",
            SignalStatus::Researched => "researched",
            SignalStatus::Surfaced => "surfaced",
            SignalStatus::Promoted => "promoted",
            SignalStatus::Dismissed => "dismissed",
        }
    }

    /// Whether a transition from `self` to `to` is legal. One forward step along
    /// the pipeline, or a jump to `Dismissed` from any non-terminal state.
    /// `Promoted` and `Dismissed` are terminal.
    pub fn allows(self, to: SignalStatus) -> bool {
        use SignalStatus::*;
        match self {
            New => matches!(to, Triaged | Dismissed),
            Triaged => matches!(to, Researched | Dismissed),
            Researched => matches!(to, Surfaced | Dismissed),
            Surfaced => matches!(to, Promoted | Dismissed),
            Promoted | Dismissed => false,
        }
    }

    /// Progress rank for the merge-on-save conflict rule: because the
    /// lifecycle is forward-only, a higher rank IS the more recent state —
    /// no timestamp needed. `Promoted` outranks `Dismissed` deliberately:
    /// a promotion already spawned a roadmap chunk, and losing that marker
    /// while the chunk exists is worse than losing a concurrent dismissal.
    fn merge_rank(self) -> u8 {
        use SignalStatus::*;
        match self {
            New => 0,
            Triaged => 1,
            Researched => 2,
            Surfaced => 3,
            Dismissed => 4,
            Promoted => 5,
        }
    }
}

/// One agent enrichment record, populated by `signal_research`.
/// Mirrors the envelope's `Enrichment`: the reasoning step that
/// produced it, the sources consulted, a summary, a confidence, and a stamp.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Enrichment {
    /// The `think_*` step number that produced this enrichment, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub think_step: Option<u32>,
    /// External references consulted (URLs, doc ids, ministr symbol ids).
    #[serde(default)]
    pub sources: Vec<String>,
    pub summary: String,
    /// Agent confidence in this enrichment, 0..1.
    pub confidence: f64,
    /// RFC 3339 timestamp.
    pub at: String,
}

/// A stakeholder signal — the local subset of the v1 envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Signal {
    /// Stable unique id (UUIDv4 minted on capture).
    pub id: String,
    pub kind: SignalKind,
    /// Human-facing attribution (display name / handle / email).
    pub from: String,
    pub body: String,
    /// Structured body (contract `$defs/StructuredContent`); `body` stays the
    /// plain fallback.
    // Advertised as a plain object in `outputSchema` (`schemars(with)`), and
    // this doc comment is deliberately one line: both are copied into each of
    // the seven Signal-shaped outputSchemas, where the full shape cost ~3 KB
    // apiece against the wire budget. The shape's one home is the contract
    // file; writers meet it at [`crate::content::parse_optional`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<serde_json::Value>")]
    pub content: Option<crate::content::StructuredContent>,
    /// RFC 3339 creation timestamp.
    pub created: String,
    pub status: SignalStatus,
    /// Append-only agent enrichment trail.
    #[serde(default)]
    pub enrichment: Vec<Enrichment>,
    /// Typed cross-refs into the think/ship/roadmap graph in `prefix:value`
    /// wire form (the `signal:` variant is written by the promotion path).
    #[serde(default)]
    pub cross_refs: Vec<String>,
    /// LOCAL-only surfacing state: when the signal was last raised
    /// to the human (RFC 3339). Set by `signal_surface`; excludes the signal
    /// from `signal_pending` so it isn't re-raised. Not part of the cloud
    /// envelope subset — the backend ignores it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surfaced_at: Option<String>,
    /// LOCAL-only surfacing state: suppress from `signal_pending`
    /// until this RFC 3339 instant passes. Set by `signal_snooze`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snooze_until: Option<String>,
    /// LOCAL-only origin stamp (store-prune-think-signal): which project
    /// captured this signal. Stamped at capture; `None` on every signal
    /// recorded before the stamp existed, which is the unprovable-origin case
    /// and means `prune` will keep it unless an operator names it explicitly.
    ///
    /// Not part of the cloud envelope subset — the backend neither sends nor
    /// reads it, matching `surfaced_at` / `snooze_until`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
}

/// The persisted per-project signal store — a CACHE of the cloud
/// system-of-record. Serialized whole to
/// `signal/sessions/<project_id>.json`, mirroring `roadmap::domain::Roadmap`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Signals {
    pub project_id: String,
    #[serde(default)]
    pub signals: Vec<Signal>,
}

/// Union `disk` into a copy of `memory` for the locked merge-on-save
/// discipline (family-stores-merge-on-save). Signals are keyed by `id`; on a
/// conflict the copy that has progressed further wins, judged by
/// (lifecycle rank, enrichment count) — both monotonic (the status table is
/// forward-only and the enrichment trail is append-only), so "further along"
/// is a sound recency proxy without any timestamp. Ties keep memory (the
/// mutation just acked by the saving process must stick). Disk-only signals
/// surviving the union is the anti-clobber property. Known caveat
/// (documented): same-rank field edits to the SAME signal (e.g. two
/// concurrent `surface` stamps) keep only one side — the LWW tradeoff the
/// think merge also accepted.
pub fn merge_signal_stores(memory: &Signals, disk: Signals) -> Signals {
    let mut merged = memory.clone();
    for disk_signal in disk.signals {
        match merged.signals.iter_mut().find(|s| s.id == disk_signal.id) {
            Some(mem_signal) => {
                if signal_wins(&disk_signal, mem_signal) {
                    *mem_signal = disk_signal;
                }
            }
            None => merged.signals.push(disk_signal),
        }
    }
    merged
}

/// THE one conflict rule for two copies of the same signal: `incoming` wins
/// only when it has strictly progressed — judged by (lifecycle rank,
/// enrichment count), both monotonic, so "further along" IS "newer" without a
/// timestamp. Ties keep `existing`. Shared by the disk merge
/// ([`merge_signal_stores`]) and the cloud reconcile
/// (`SignalEngine::upsert_signal`) so a stale cloud copy can never roll back
/// a fresher local transition (reconcile-recency-guard).
#[must_use]
pub fn signal_wins(incoming: &Signal, existing: &Signal) -> bool {
    (incoming.status.merge_rank(), incoming.enrichment.len())
        > (existing.status.merge_rank(), existing.enrichment.len())
}

#[cfg(test)]
#[path = "records.tests.rs"]
mod tests;
