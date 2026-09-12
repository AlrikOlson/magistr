//! The roadmap verbs an attached client can ask its owner for.

use magistr_workflow::content::StructuredContent;
use magistr_workflow::roadmap::domain::{BlockerKind, ChunkStatus};
use serde::{Deserialize, Serialize};

/// One roadmap mutation, argument for argument the
/// [`NativeEngines`](super::NativeEngines) verb of the same name.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "verb", rename_all = "snake_case")]
pub enum RoadmapCommand {
    /// `roadmap_start_chunk`.
    StartChunk { id: String },
    /// `roadmap_complete_chunk`.
    CompleteChunk {
        id: String,
        ship_ref: Option<String>,
    },
    /// `roadmap_add_chunk`.
    AddChunk {
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
    },
    /// `roadmap_update_chunk`.
    UpdateChunk {
        id: String,
        title: Option<String>,
        priority: Option<u32>,
        description: Option<String>,
        acceptance: Option<Vec<String>>,
        deps: Option<Vec<String>>,
        content: Option<StructuredContent>,
    },
    /// `roadmap_clear_blocked_by`.
    ClearBlockedBy { id: String },
    /// `roadmap_set_blocked_by`.
    SetBlockedBy {
        id: String,
        kind: BlockerKind,
        reason: String,
        evidence: Option<String>,
    },
    /// `roadmap_set_status`.
    SetStatus { id: String, to: ChunkStatus },
    /// `roadmap_obsolete_chunk`.
    ObsoleteChunk { id: String, reason: String },
    /// `roadmap_reprioritize`.
    Reprioritize {
        id: String,
        suggested_priority: u32,
        reason: String,
    },
    /// `roadmap_propose_title`.
    ProposeTitle {
        id: String,
        suggested_title: String,
        reason: String,
        source: String,
    },
    /// `roadmap_propose_status`.
    ProposeStatus {
        id: String,
        suggested_status: ChunkStatus,
        reason: String,
        source: String,
    },
    /// `roadmap_link`.
    Link { id: String, cross_ref: String },
    /// `roadmap_set_group`.
    SetGroup { id: String, group: Option<String> },
    /// `roadmap_resolve_title_proposal`.
    ResolveTitleProposal { id: String, accept: bool },
    /// `roadmap_resolve_status_proposal`.
    ResolveStatusProposal { id: String, accept: bool },
    /// `roadmap_resolve_reprioritize`.
    ResolveReprioritize { id: String, accept: bool },
}
