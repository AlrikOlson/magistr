use magistr_workflow_contracts::Observed;
use std::sync::Arc;

use rmcp::model::JsonObject;
use schemars::{JsonSchema, schema_for};
use serde::Serialize;
use serde_json::Value;

#[derive(Serialize, JsonSchema)]
pub struct GoalOutput {
    pub description: String,
    pub acceptance_criteria: Vec<String>,
    pub constraints: Vec<String>,
    pub scope: String,
    pub status: String,
    pub project_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct PlanOutput {
    pub steps: Vec<StepSummary>,
    pub total: usize,
}

#[derive(Serialize, JsonSchema)]
pub struct StepSummary {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub step_type: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimate: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct StepOutput {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub step_type: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    pub progress_count: usize,
    pub checks_count: usize,
    pub artifacts_count: usize,
}

#[derive(Serialize, JsonSchema)]
pub struct ProgressOutput {
    pub id: u32,
    pub step_id: String,
    pub timestamp: String,
    #[serde(rename = "type")]
    pub progress_type: String,
    pub description: String,
    pub files_touched: Vec<String>,
    pub tools_used: Vec<String>,
    pub result: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub think_step: Option<u32>,
}

#[derive(Serialize, JsonSchema)]
pub struct CheckOutput {
    #[serde(rename = "type")]
    pub check_type: String,
    pub name: String,
    pub passed: bool,
    pub details: String,
    pub required: bool,
    /// `true` when `passed` came from a command the server actually ran.
    pub verified: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    /// What happened with the requested report, when one was requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report: Option<crate::run::report::ReportRecord>,
    /// Parsed summary — additive; the exit code stays the source of truth.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub results: Option<crate::run::report::TestResults>,
    pub timestamp: String,
}

#[derive(Serialize, JsonSchema)]
pub struct RunReport {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    pub steps: StepCounts,
    pub artifacts_count: usize,
    pub warnings: Vec<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct StepCounts {
    pub total: usize,
    pub done: usize,
}

#[derive(Serialize, JsonSchema)]
pub struct StatusOutput {
    pub project_id: String,
    pub execution: Option<crate::run::application::ExecutionBinding>,
    pub run_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub goal: Option<GoalOutput>,
    pub steps: StatusCounts,
    pub step_list: Vec<StepSummary>,
    pub recent_progress: Vec<ProgressOutput>,
    pub checks: Vec<CheckRef>,
    pub artifacts: Vec<ArtifactRef>,
    pub think_refs: Vec<ThinkRef>,
}

#[derive(Serialize, JsonSchema)]
pub struct StatusCounts {
    pub planned: usize,
    pub active: usize,
    pub waiting: usize,
    pub done: usize,
    pub skipped: usize,
    pub total: usize,
}

#[derive(Serialize, JsonSchema)]
pub struct CheckRef {
    pub step_id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub check_type: String,
    pub passed: bool,
    pub required: bool,
}

#[derive(Serialize, JsonSchema)]
pub struct ArtifactRef {
    pub step_id: String,
    #[serde(rename = "type")]
    pub artifact_type: String,
    #[serde(rename = "ref")]
    pub reference: String,
    pub description: String,
}

#[derive(Serialize, JsonSchema)]
pub struct ThinkRef {
    pub step_id: String,
    pub ref_type: String,
    pub value: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress_id: Option<u32>,
}

#[derive(Serialize, JsonSchema)]
pub struct ExportOutput {
    pub format: String,
    pub trace: String,
}

#[derive(Serialize, JsonSchema)]
pub struct ClearOutput {
    pub status: String,
}

#[derive(Serialize, JsonSchema)]
pub struct StructuredError {
    pub error_kind: String,
    pub message: String,
}

/// `approval_open` — either the opened pending gate, or the immediate
/// default resolution when no workspace could carry it (headless-safe).
#[derive(Serialize, JsonSchema)]
pub struct ApprovalOpenOutput {
    pub opened: bool,
    /// `pending` when opened; `unopened` when the default applied at once.
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub question: Option<String>,
    /// The declared safe default (the option key expiry resolves to).
    #[serde(rename = "default", skip_serializing_if = "Option::is_none")]
    pub default_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    /// Set when `opened` is false: the default that applies.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

/// `approval_wait` — the gate's resolution at this moment.
#[derive(Serialize, JsonSchema)]
pub struct ApprovalWaitOutput {
    /// `answered` | `expired` | `pending`.
    pub state: String,
    /// The decided (or defaulted) option key, absent while pending.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub choice: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decided_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decided_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seconds_left: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

pub fn output_schema_for(tool_name: &str) -> Option<Arc<JsonObject>> {
    let value: Value = match tool_name {
        "run_open" => schema_for!(Observed<GoalOutput>).to_value(),
        "arrange_steps" => schema_for!(Observed<PlanOutput>).to_value(),
        "start_step" => schema_for!(Observed<StepOutput>).to_value(),
        "record_progress" => schema_for!(Observed<ProgressOutput>).to_value(),
        "finish_step" => schema_for!(Observed<StepOutput>).to_value(),
        "run_block_step" => schema_for!(Observed<StepOutput>).to_value(),
        "check_step" => schema_for!(crate::run::mcp::check_step_output::CheckStepOutput).to_value(),
        "approval_open" => schema_for!(ApprovalOpenOutput).to_value(),
        "approval_wait" => schema_for!(ApprovalWaitOutput).to_value(),
        "run_finish" => schema_for!(Observed<RunReport>).to_value(),
        "run_compact_checks" => {
            schema_for!(crate::run::check_result_compaction::CheckResultCompaction).to_value()
        }
        "run_status" => schema_for!(StatusOutput).to_value(),
        "run_export" => schema_for!(ExportOutput).to_value(),
        "run_reset" => schema_for!(Observed<ClearOutput>).to_value(),
        _ => return None,
    };
    match value {
        Value::Object(map) => Some(Arc::new(map)),
        _ => None,
    }
}
