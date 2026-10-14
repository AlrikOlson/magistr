//! Output schemas for the small mutating tools — `think_pin_step`,
//! `think_revise_estimate`, `think_set_branch_status`, and
//! `think_wipe_trace`. Each emits a thin acknowledgment shape;
//! collected here because none warrants its own file.

use schemars::JsonSchema;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct PinStepOutput {
    pub source: magistr_workflow_contracts::ReadSource,
    pub step_number: u32,
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
pub struct SetBranchStatusOutput {
    pub source: magistr_workflow_contracts::ReadSource,
    pub branch_id: String,
    /// One of: `"active"`, `"merged"`, `"abandoned"`.
    pub previous_status: String,
    /// One of: `"active"`, `"merged"`, `"abandoned"`.
    pub new_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merged_into: Option<u32>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct WipeTraceOutput {
    pub source: magistr_workflow_contracts::ReadSource,
    pub cleared: bool,
}
