//! Output schemas for every MCP tool that returns structured JSON.
//!
//! These structs exist solely so `schemars::schema_for!(T)` can compute a
//! JSON Schema we attach to each `Tool.output_schema`. The engine emits
//! `serde_json::Value` directly; we never deserialize back into these
//! types at runtime. Field shapes here must therefore stay aligned with
//! the JSON the engine actually produces.
//!
//! `outputSchema` is what lets a client validate `structuredContent` and an
//! agent pattern-match without parsing prose, and the
//! [2025-06-18 MCP spec](https://modelcontextprotocol.io/specification/2025-06-18/server/tools)
//! requires a server that advertises one to emit conformant
//! `structuredContent`.
//!
//! This module used to claim, flatly, that "a `tools/list` response in
//! 2026-style MCP carries `outputSchema` on every tool." **That was never true
//! of this server** — 15 of 48 tools carried none — and the claim is what made
//! the gap invisible: it read as a description of the code rather than the
//! aspiration it was (mcp-outputschema-inconsistency). Corrected here, because
//! a doc that overstates the invariant is worse than no doc; it stops anyone
//! from checking.
//!
//! The real invariant, and it IS enforced — by
//! `every_tool_is_dispositioned_for_output_schema` in
//! `tests/unified_service.rs` — is weaker and honest: **every tool either
//! carries a schema or is declared**, in
//! [`crate::mcp::unified::SCHEMA_EXEMPT`] (a shape that genuinely cannot be
//! described) or [`crate::mcp::unified::SCHEMA_PENDING_BUDGET`] (a known gap,
//! which must shrink and never grow). A tool with neither fails the build.
//!
//! One file per response shape (or one file per family of small shapes —
//! see [`mutations`]). The [`output_schema_for`] dispatcher in this module
//! maps tool names to compiled `JsonSchema` blobs.

use std::sync::Arc;

use schemars::schema_for;
use serde_json::Value;

/// JSON object alias matching `rmcp::model::JsonObject`. Re-exported by
/// the wire adapter when rmcp is plugged in.
pub type JsonObject = serde_json::Map<String, Value>;

pub mod engine_status;
pub mod error_envelope;
pub mod impact;
pub mod mutations;
pub mod record;
pub mod review;
pub mod search;

pub use engine_status::{EngineStatusOutput, LogDescriptor, PinnedNoteDescriptor};
pub use error_envelope::StructuredError;
pub use impact::{
    ImpactByRelation, ImpactDownstream, ImpactOutput, ImpactThreadFork, ImpactUpstream,
};
pub use mutations::{PinOutput, ReviseEstimateOutput, SetThreadStatusOutput, WipeOutput};
pub use record::{RecentNoteRollup, RecordOutput, ThreadEcho, ThreadSummary};
pub use review::{
    CheckpointHypothesis, CheckpointRefutedChain, CheckpointRevisedUndefended,
    CheckpointStaleThread, ReviewOutput,
};
pub use search::{SearchHit, SearchOutput};

use crate::decision::domain::Note;

/// Return the JSON Schema for a tool's structuredContent, keyed by tool
/// name. `None` for tools this family does not serve, and for the one
/// declared exemption — `decision_export`, whose output is format-dependent
/// text. See `SCHEMA_EXEMPT`.
pub fn output_schema_for(tool_name: &str) -> Option<Arc<JsonObject>> {
    let value: Value = match tool_name {
        "decision_record" => schema_for!(RecordOutput).to_value(),
        "decision_status" => schema_for!(EngineStatusOutput).to_value(),
        "decision_get" => schema_for!(Note).to_value(),
        "decision_logs" => {
            schema_for!(magistr_workflow_contracts::Observed<Vec<LogDescriptor>>).to_value()
        }
        "decision_log" => {
            schema_for!(magistr_workflow_contracts::Observed<Option<crate::decision::domain::Log>>)
                .to_value()
        }
        "decision_search" => schema_for!(SearchOutput).to_value(),
        "decision_impact" => schema_for!(ImpactOutput).to_value(),
        "decision_pin" => schema_for!(PinOutput).to_value(),
        "decision_revise_estimate" => schema_for!(ReviseEstimateOutput).to_value(),
        "decision_set_thread_status" => schema_for!(SetThreadStatusOutput).to_value(),
        "decision_review" => schema_for!(ReviewOutput).to_value(),
        "decision_wipe" => schema_for!(WipeOutput).to_value(),
        _ => return None,
    };
    match value {
        Value::Object(map) => Some(Arc::new(map)),
        _ => None,
    }
}
