//! Output schemas for the `request_*` tools.
//!
//! # Why this module exists
//!
//! It didn't, and that was the whole defect. `decision_*` and `plan_*` each
//! carry an `output_schemas` module and patch `Tool.output_schema` in their
//! `list_tools_view`; `request_*` had neither, so all ten of its tools shipped
//! with no schema at all while their siblings shipped with one. Nothing said
//! so — absence is indistinguishable from an oversight, which is exactly how
//! ten tools got here (mcp-outputschema-inconsistency).
//!
//! # The shapes are the handlers', not an idealization of them
//!
//! As in [`crate::decision::output_schemas`], these structs exist solely so
//! `schemars::schema_for!(T)` can compute a blob. The engine emits
//! `serde_json::Value` directly and we never deserialize back into these
//! types, so a field that drifts from what the handler actually builds is a
//! silent lie to every client that validates against it. Each type below
//! names the handler it mirrors for that reason.
//!
//! Seven of the ten tools return a bare [`Request`], so they share its schema
//! rather than wrapping it in ten near-identical envelopes.

use std::sync::Arc;

use schemars::{JsonSchema, schema_for};
use serde::Serialize;
use serde_json::Value;

use crate::decision::output_schemas::JsonObject;
use crate::request::domain::Request;
use magistr_workflow_contracts::Observed;

/// `request_status` — mirrors `RequestEngine::status`.
#[derive(Debug, Serialize, JsonSchema)]
pub struct RequestStatusOutput {
    /// Project this inbox belongs to.
    pub project_id: String,
    pub counts: RequestCounts,
    /// Newest-first digest, capped at `RequestEngine::STATUS_LIST_CAP`.
    pub requests: Vec<RequestDigest>,
    /// How many requests the cap left out.
    pub omitted: usize,
}

/// Per-status tallies. `total` is every request, not the sum of a subset.
#[derive(Debug, Serialize, JsonSchema)]
pub struct RequestCounts {
    pub new: usize,
    pub triaged: usize,
    pub researched: usize,
    pub raised: usize,
    pub promoted: usize,
    pub dismissed: usize,
    pub total: usize,
}

/// The trimmed request `request_status` lists — deliberately NOT a full
/// [`Request`]: `body` is truncated and enrichments/cross-refs are omitted, so
/// a client must not expect the full record here.
#[derive(Debug, Serialize, JsonSchema)]
pub struct RequestDigest {
    pub id: String,
    pub kind: String,
    pub status: String,
    /// Who raised it, when recorded.
    pub from: Option<String>,
    /// Truncated. Fetch the full text with `request_get`.
    pub body: String,
    pub created: String,
}

/// `request_ready` — the inbox slice above the confidence floor.
#[derive(Debug, Serialize, JsonSchema)]
pub struct RequestReadyOutput {
    /// Length of `requests` after the limit was applied — NOT the total number
    /// pending, so it cannot be used to decide whether more remain.
    pub count: usize,
    pub requests: Vec<Request>,
}

/// Each owner acknowledges its own committed value and save source.
#[derive(Debug, Serialize, JsonSchema)]
pub struct RequestPromoteOutput {
    pub request_id: String,
    pub task_id: String,
    pub created: bool,
    pub task: Observed<Value>,
    pub request: Observed<Request>,
}

/// The JSON Schema for a `request_*` tool's `structuredContent`, or `None` for
/// a name this family does not serve.
///
/// Every `request_*` tool is covered. There is no exemption in this family —
/// see [`crate::mcp::unified::SCHEMA_EXEMPT`] for the one tool server-wide
/// that legitimately has no single output shape.
pub fn output_schema_for(tool_name: &str) -> Option<Arc<JsonObject>> {
    let value: Value = match tool_name {
        // The seven that return the request itself.
        "request_get" => schema_for!(Request).to_value(),
        "request_capture" | "request_link" | "request_research" | "request_raise"
        | "request_snooze" | "request_dismiss" => schema_for!(Observed<Request>).to_value(),
        "request_status" => schema_for!(RequestStatusOutput).to_value(),
        "request_ready" => schema_for!(RequestReadyOutput).to_value(),
        "request_promote" => schema_for!(RequestPromoteOutput).to_value(),
        _ => return None,
    };
    match value {
        Value::Object(map) => Some(Arc::new(map)),
        _ => None,
    }
}
