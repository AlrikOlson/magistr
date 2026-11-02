//! Output schema for `decision_record`.

use schemars::JsonSchema;
use serde::Serialize;

use crate::decision::domain::DepEdge;

/// What the engine returns after recording one reasoning note. Mirrors
/// the `response` map built at the end of `ReasoningServer::process_note`.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct RecordOutput {
    pub source: magistr_workflow_contracts::ReadSource,
    pub record_id: String,
    pub note: u32,
    pub estimated_total: u32,
    pub total_notes: u32,

    /// Only present when the log's final-note flag is set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed: Option<bool>,

    /// The note number that this note revises, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revised_note: Option<u32>,

    /// Thread this note belongs to. Present when the note is on a thread.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread: Option<ThreadEcho>,

    /// First ~120 chars of `thought`, server-truncated for confirmation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thought_excerpt: Option<String>,

    /// First ~120 chars of `outcome`, server-truncated for confirmation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome_excerpt: Option<String>,

    /// Echoed back so the model knows declared deps were accepted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dependencies: Option<Vec<DepEdge>>,

    /// Tools the model declared it used on this note.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools_recorded: Option<Vec<String>>,

    /// Soft advisories about this note (low confidence upstream, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warnings: Option<Vec<String>>,

    /// Rolling window of prior notes, pinned-aware and size-budgeted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recent_notes: Option<Vec<RecentNoteRollup>>,

    /// What the rollup budget dropped. Present only when pinned notes were
    /// actually withheld — a rollup that fits says nothing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recent_notes_omitted: Option<RecentNotesOmitted>,

    /// Compact summary of every active/merged/abandoned thread.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threads_summary: Option<Vec<ThreadSummary>>,
}

/// Summary-plus-handle for the pinned notes the rollup budget withheld.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct RecentNotesOmitted {
    /// How many pinned notes were dropped to stay inside the size budget.
    pub pinned_withheld: u32,
    /// The tool that fetches any of them back, by note_number.
    pub fetch_with: String,
    /// Human-readable explanation of the trade.
    pub note: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ThreadEcho {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<u32>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct RecentNoteRollup {
    /// Note number.
    pub n: u32,
    pub purpose: String,
    pub thought_excerpt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rationale_excerpt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revised_by: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinned: Option<bool>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ThreadSummary {
    pub id: String,
    pub name: String,
    pub from_note: u32,
    /// Count of notes on this thread.
    pub notes: u32,
    /// One of: `"open"`, `"merged"`, `"abandoned"`.
    pub status: String,
    pub depth: u32,
}
