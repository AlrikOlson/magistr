//! Output schema for `decision_status`.

use schemars::JsonSchema;
use serde::Serialize;

use crate::mcp::client_view::ClientView;

/// Engine introspection — config, counts, version, optionally per-log.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct EngineStatusOutput {
    pub persistence_enabled: bool,
    pub data_dir: String,
    pub logs_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_log_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_log: Option<String>,
    pub logs_count: u32,
    pub log_present: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<magistr_workflow_contracts::ReadSource>,
    /// A COUNT of retained notes — NOT the highest note number used. The
    /// numbering is sparse, so this is smaller than the head by however many
    /// notes have been trimmed. Never derive a note number from it; use
    /// [`Self::next_note_number`].
    pub total_notes: u32,
    /// The next safe `note` for `decision_record`: the highest note
    /// number recorded anywhere in this project, plus one. This is the same
    /// expression the engine uses to auto-assign an omitted number, so the
    /// published value and the value a caller actually gets cannot drift.
    ///
    /// Monotonic across history trimming (the trim drains the oldest, i.e.
    /// lowest-numbered, notes first) and across a process restart. NOT
    /// preserved by `decision_wipe`, which deletes the persisted files it
    /// would have to be read from.
    pub next_note_number: u32,
    pub threads_count: u32,
    pub pinned_count: u32,
    pub completed: bool,
    pub recent_notes_limit: u32,
    pub max_history_size: u32,
    pub strict_mode: bool,
    pub version: String,
    /// Present only when `verbose: true`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinned: Option<Vec<PinnedNoteDescriptor>>,
    /// Present only when `verbose: true`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logs: Option<Vec<LogDescriptor>>,
    /// The live MCP client on the other end of *this* request — what it
    /// declared, and where the declaration was read from. Absent when the
    /// engine is inspected outside a wire call (the CLI path has no client).
    ///
    /// This is the readable half of the capability reporting whose stderr half
    /// some hosts discard after connect.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client: Option<ClientView>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct PinnedNoteDescriptor {
    pub note: u32,
    pub purpose: String,
    pub thought_excerpt: String,
    pub outcome_excerpt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revised_by: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct LogDescriptor {
    pub log_id: Option<String>,
    pub note_count: usize,
    pub completed: bool,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}
