//! Output schemas for the small mutating tools — `decision_pin`,
//! `decision_revise_estimate`, `decision_set_thread_status`, and
//! `decision_wipe`. Each emits a thin acknowledgment shape;
//! collected here because none warrants its own file.

use schemars::JsonSchema;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct PinOutput {
    pub source: magistr_workflow_contracts::ReadSource,
    pub note: u32,
    pub was_pinned: bool,
    pub now_pinned: bool,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ReviseEstimateOutput {
    pub source: magistr_workflow_contracts::ReadSource,
    pub previous: u32,
    pub new_estimate: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SetThreadStatusOutput {
    pub source: magistr_workflow_contracts::ReadSource,
    pub thread_id: String,
    /// One of: `"open"`, `"merged"`, `"abandoned"`.
    pub previous_status: String,
    /// One of: `"open"`, `"merged"`, `"abandoned"`.
    pub new_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merged_into: Option<u32>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct WipeOutput {
    pub source: magistr_workflow_contracts::ReadSource,
    pub cleared: bool,
}
