use rmcp::{
    ErrorData, handler::server::wrapper::Parameters, model::CallToolResult, tool, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;

use super::service::ShipService;
use crate::ship::domain::action::ActionType;
use crate::ship::domain::artifact::{Artifact, ArtifactType};
use crate::ship::domain::check::CheckType;
use crate::ship::domain::objective::VerificationMode;
use crate::ship::domain::task::TaskType;

impl ShipService {
    pub(crate) fn make_tool_router() -> rmcp::handler::server::router::tool::ToolRouter<Self> {
        Self::tool_router()
    }
}

// ── Arg types ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetObjectiveArgs {
    #[serde(default)]
    #[schemars(required, !default)]
    pub description: String,
    #[serde(default, deserialize_with = "crate::infra::coerce::string_or_seq")]
    pub acceptance_criteria: Vec<String>,
    #[serde(default, deserialize_with = "crate::infra::coerce::string_or_seq")]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub scope: String,
    /// `full` (compatibility default) requires every configured project gate.
    /// Explicitly select `focused` for bounded changes, including code, with
    /// recorded checks covering their behavior, dependencies, and acceptance.
    #[serde(default)]
    pub verification: VerificationMode,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PlanArgs {
    #[serde(default)]
    #[schemars(required, !default)]
    pub action: PlanAction,
    #[serde(default)]
    #[schemars(required, !default)]
    pub task_id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub task_type: Option<TaskType>,
    #[serde(default)]
    pub estimate: Option<String>,
    #[serde(default)]
    pub after: Option<String>,
    #[serde(default)]
    pub think_branch: Option<String>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PlanAction {
    Add,
    Remove,
    Reorder,
    /// Catches a missing or unrecognized `action` value. Rather than reject
    /// with a JSON-RPC -32602 (which cancels sibling tool calls in a parallel
    /// batch), the value deserializes to this and the handler returns a soft
    /// error naming the valid actions.
    #[default]
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct StartArgs {
    #[serde(default)]
    #[schemars(required, !default)]
    pub task_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RecordArgs {
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(rename = "type", default = "default_action_type")]
    pub action_type: ActionType,
    pub description: String,
    #[serde(default, deserialize_with = "crate::infra::coerce::string_or_seq")]
    pub files_touched: Vec<String>,
    #[serde(default, deserialize_with = "crate::infra::coerce::string_or_seq")]
    pub tools_used: Vec<String>,
    #[serde(default)]
    pub result: String,
    #[serde(default)]
    pub think_step: Option<u32>,
}

fn default_action_type() -> ActionType {
    ActionType::Code
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CompleteArgs {
    pub task_id: String,
    #[serde(default)]
    pub artifacts: Vec<ArtifactInput>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ArtifactInput {
    #[serde(rename = "type")]
    pub artifact_type: ArtifactType,
    #[serde(rename = "ref")]
    pub reference: String,
    #[serde(default)]
    pub description: String,
}

impl From<ArtifactInput> for Artifact {
    fn from(input: ArtifactInput) -> Self {
        Self {
            artifact_type: input.artifact_type,
            reference: input.reference,
            description: input.description,
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BlockArgs {
    pub task_id: String,
    pub reason: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CheckArgs {
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(rename = "type")]
    pub check_type: CheckType,
    pub name: String,
    /// Self-reported pass/fail. Optional when `command` is given (the real exit
    /// code decides). Required otherwise.
    #[serde(default)]
    pub passed: Option<bool>,
    #[serde(default)]
    pub details: String,
    #[serde(default = "default_true")]
    pub required: bool,
    /// A shell command to actually run (e.g. "cargo test"). When present the
    /// server executes it, and `passed`/`exit_code`/`details` are taken from the
    /// real result — the check is `verified` and cannot be faked.
    #[serde(default)]
    pub command: Option<String>,
    /// A machine-readable report the runner was asked to emit, read after
    /// `command` runs. Additive: a report problem never fails the check.
    #[serde(default)]
    pub report: Option<ReportArg>,
}

/// Where the runner was asked to write its machine-readable report.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReportArg {
    /// Report format key. Supported: "junit".
    pub format: String,
    /// Report file path, relative to the project directory.
    pub path: String,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ShipArgs {
    #[serde(default)]
    pub artifacts: Vec<ArtifactInput>,
    #[serde(default)]
    pub summary: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExportArgs {
    #[serde(default = "default_format")]
    pub format: String,
}

fn default_format() -> String {
    "markdown".to_string()
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct NoArgs {}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GateOpenArgs {
    /// The one plain-language sentence a human answers.
    #[serde(default)]
    #[schemars(required, !default)]
    pub question: String,
    /// Plain-prose context: what happens on each answer, what was verified.
    #[serde(default)]
    pub body: String,
    /// Array of {key, label} choices. Omitted = yes/no.
    #[serde(default)]
    pub options: serde_json::Value,
    /// The safe answer an unanswered gate resolves to at expiry. REQUIRED.
    #[serde(default, rename = "default")]
    #[schemars(required, !default)]
    pub default_key: Option<String>,
    /// Seconds until the gate answers itself with the default (default 3600).
    #[serde(default)]
    pub timeout_secs: Option<i64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GateWaitArgs {
    #[serde(default)]
    #[schemars(required, !default)]
    pub gate_id: String,
    /// Seconds this ONE call may wait before returning `pending` (default 25,
    /// max 55 — always under the MCP client's own call timeout; loop to keep
    /// waiting).
    #[serde(default)]
    pub wait_secs: Option<i64>,
}

/// The command execution budget, shared with MCP task retention.
pub const CHECK_TIMEOUT_SECS: u64 = crate::ship::application::command_runner::CHECK_TIMEOUT_SECS;

#[cfg(test)]
use crate::ship::application::command_runner::{CHECK_OUTPUT_TAIL, run_check_command, tail_chars};

// ── Tool handlers ──────────────────────────────────────────────────

#[tool_router]
impl ShipService {
    #[tool(
        name = "ship_set_objective",
        description = "Define the objective before planning tasks. Choose focused verification for bounded changes, including code: all recorded acceptance checks must pass, while unrelated project gates are omitted. Full is the compatibility default; use it only for an explicit full-suite request or concrete cross-cutting risk. Updating an unfinished objective preserves tasks. Replacing a completed or abandoned objective starts a fresh cycle and clears old tasks, reported as cleared_stale_tasks. Criteria and constraints accept arrays or a single string.",
        annotations(
            title = "Set objective",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn ship_set_objective(
        &self,
        Parameters(args): Parameters<SetObjectiveArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::application_result(self.use_cases.set_objective_with_verification(
            args.description,
            args.acceptance_criteria,
            args.constraints,
            args.scope,
            args.verification,
        ))
    }

    #[tool(
        name = "ship_plan",
        description = "Add, remove, or reorder tasks in the execution plan.\n\nInputs: action ('add'|'remove'|'reorder'), task_id (required), title (required for add), task_type ('implement'|'test'|'review'|'config'|'docs'|'research'), estimate ('trivial'|'small'|'medium'|'large'), after (task_id to place after, for reorder), think_branch (optional cross-ref to a think_* branch).\n\nReturns: the updated task list.\n\nPitfalls: task_id must be unique within the objective for 'add'. Cannot remove an active or completed task.",
        annotations(
            title = "Plan tasks",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false,
        )
    )]
    pub async fn ship_plan(
        &self,
        Parameters(args): Parameters<PlanArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        use crate::ship::application::PlanChange;
        let change = match args.action {
            PlanAction::Add => PlanChange::Add {
                id: args.task_id,
                title: args.title,
                task_type: args.task_type,
                estimate: args.estimate,
                think_branch: args.think_branch,
            },
            PlanAction::Remove => PlanChange::Remove { id: args.task_id },
            PlanAction::Reorder => PlanChange::Reorder {
                id: args.task_id,
                after: args.after,
            },
            PlanAction::Unknown => PlanChange::Unknown,
        };
        Self::typed_result(self.use_cases.change_plan(change))
    }

    #[tool(
        name = "ship_start",
        description = "Begin work on a task. Sets its status to active and records the start time. Only one task can be active at a time.\n\nInputs: task_id (required).\n\nReturns: the started task.\n\nPitfalls: fails if another task is already active. Complete or block the current task first.",
        annotations(
            title = "Start task",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false,
        )
    )]
    pub async fn ship_start(
        &self,
        Parameters(args): Parameters<StartArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::application_result(self.use_cases.start_task(&args.task_id))
    }

    #[tool(
        name = "ship_record",
        description = "Log an action within the active task. This is the primary workhorse — call it every time you do something: write code, run a command, make a decision, research something.\n\nInputs: task_id (optional — defaults to active task), type ('code'|'test'|'debug'|'research'|'config'|'refactor'|'review'), description (required), files_touched (string[]), tools_used (string[]), result (string), think_step (optional u32 — cross-ref to the think_* step that motivated this action).\n\nReturns: the recorded action with its assigned id.\n\nPitfalls: if no task is active and no task_id is provided, the call fails.",
        annotations(
            title = "Record action",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false,
        )
    )]
    pub async fn ship_record(
        &self,
        Parameters(args): Parameters<RecordArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::application_result(self.use_cases.record_action(
            args.task_id.as_deref(),
            args.action_type,
            args.description,
            args.files_touched,
            args.tools_used,
            args.result,
            args.think_step,
        ))
    }

    #[tool(
        name = "ship_complete",
        description = "Close a task and record what was produced.\n\nInputs: task_id (required), artifacts (array of {type, ref, description}).\n\nReturns: the completed task.\n\nPitfalls: can only complete a task that is active or blocked.",
        annotations(
            title = "Complete task",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false,
        )
    )]
    pub async fn ship_complete(
        &self,
        Parameters(args): Parameters<CompleteArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let artifacts = args.artifacts.into_iter().map(Artifact::from).collect();
        Self::application_result(self.use_cases.complete_task(&args.task_id, artifacts))
    }

    #[tool(
        name = "ship_block",
        description = "Mark a task as blocked with a reason.\n\nInputs: task_id (required), reason (required).\n\nReturns: the blocked task.\n\nPitfalls: can only block an active task.",
        annotations(
            title = "Block task",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false,
        )
    )]
    pub async fn ship_block(
        &self,
        Parameters(args): Parameters<BlockArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::application_result(self.use_cases.block_task(&args.task_id, args.reason))
    }

    #[tool(
        name = "ship_check",
        description = "Record a quality check on task_id (defaults to the active task). PREFER passing `command`: the server runs it and records the real exit code, details and verified:true. Without command, passed is required and remains self-reported; use this only for checks that cannot run as commands. Optional report {format:'junit', path} parses a runner-produced report into counts and failing test names. A missing or malformed report does not change pass/fail or verified; the response reports the parsing error. Request a report from the runner instead of scraping stdout. Returns the recorded check and report/results when requested. Failed required checks and self-reported passes are flagged at finalization.",
        annotations(
            title = "Record check",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false,
        )
    )]
    pub async fn ship_check(
        &self,
        Parameters(args): Parameters<CheckArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        use crate::ship::application::{CheckRequest, CheckSettlement, ShipUseCases};
        let request = CheckRequest {
            task_id: args.task_id,
            check_type: args.check_type,
            name: args.name,
            passed: args.passed,
            details: args.details,
            required: args.required,
            command: args.command,
            report: args.report.map(|report| (report.format, report.path)),
        };
        let admission = match self.use_cases.admit_check(request) {
            Ok(admission) => admission,
            Err(error) => return Self::typed_result::<()>(Err(error)),
        };
        let outcome = ShipUseCases::execute_check(&admission).await;
        match self.use_cases.settle_check(admission, outcome) {
            Ok(CheckSettlement::Applied { check, .. }) => Self::application_result(Ok(check)),
            Ok(CheckSettlement::Stale {
                admission,
                outcome,
                receipt,
            }) => Ok(Self::ok_structured(serde_json::json!({
                "ok": false, "error_kind": "stale_result",
                "message": "The admitted project/run/task changed while the check ran; its result was not reassigned. Admission, outcome and the settled receipt are returned for recovery.",
                "admission": admission, "outcome": outcome, "receipt": receipt,
            }))),
            Err(error) => Self::application_result::<()>(Err(error)),
        }
    }

    #[tool(
        name = "ship_finalize",
        description = "Mark the objective as completed and record final artifacts (commits, PRs, deployments). Reviews all checks and warns about any required checks that failed or are missing.\n\nInputs: artifacts (array of {type, ref, description}), summary (optional string).\n\nReturns: ship report — objective status, task completion stats, check summary, warnings about failed/missing required checks.\n\nPitfalls: does NOT block on failed checks — it warns. The trace records whether the agent shipped with failures.",
        annotations(
            title = "Ship objective",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false,
        )
    )]
    pub async fn ship_finalize(
        &self,
        Parameters(args): Parameters<ShipArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let artifacts = args.artifacts.into_iter().map(Artifact::from).collect();
        Self::application_result(self.use_cases.finalize(artifacts, args.summary))
    }

    #[tool(
        name = "ship_gate_open",
        description = "Open an approval gate: pause-worthy work that needs a human yes, answered from the workspace webapp. Creates a ship/gate record in the cloud workspace and returns immediately with the gate id — then poll ship_gate_wait. HEADLESS-SAFE BY CONSTRUCTION: `default` is required, the gate answers itself with that default at `timeout_secs`, and with no cloud workspace connected the call resolves to the default at once instead of opening anything.\n\nInputs: question (required — one plain sentence), body (plain-prose context), options (array of {key,label}; omitted = yes/no), default (required — the option key an unanswered gate resolves to), timeout_secs (default 3600, clamped 30..604800).\n\nReturns: {opened:true, gate_id, expires_at, ...} or {opened:false, resolved:<default>, reason} when no workspace could carry the gate.\n\nPitfalls: pick the SAFE default (usually 'no'/'hold') — it is what headless expiry applies.",
        annotations(
            title = "Open approval gate",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true,
        )
    )]
    pub async fn ship_gate_open(
        &self,
        Parameters(args): Parameters<GateOpenArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        use crate::ship::application::ApprovalGateRequest;
        let options = if args.options.is_null() {
            None
        } else {
            match serde_json::from_value(args.options) {
                Ok(options) => Some(options),
                Err(error) => {
                    return Ok(Self::err_structured(
                        "invalid_args",
                        format!("`options` must be an array of {{key, label}} objects: {error}"),
                    ));
                }
            }
        };
        Self::typed_result(
            self.use_cases
                .open_gate(ApprovalGateRequest {
                    question: args.question,
                    body: args.body,
                    options,
                    default_key: args.default_key,
                    timeout_secs: args.timeout_secs,
                })
                .await,
        )
    }

    #[tool(
        name = "ship_gate_wait",
        description = "Check (and briefly wait on) an open approval gate. Polls the cloud workspace; returns as soon as the gate is answered or expired, else after wait_secs with state 'pending' — loop on pending to keep waiting. The wait is BOUNDED (max 55s per call, under any MCP client timeout); an unanswered gate deterministically resolves to its declared default once expires_at passes, so a headless session always completes.\n\nInputs: gate_id (required — from ship_gate_open), wait_secs (default 25, clamped 0..55).\n\nReturns: {state:'answered', choice, decided_by, decided_at, note?} | {state:'expired', choice:<default>} | {state:'pending', seconds_left}.\n\nPitfalls: if the workspace is unreachable, this returns a soft error — apply your declared default yourself once expires_at passes.",
        annotations(
            title = "Wait on approval gate",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = true,
        )
    )]
    pub async fn ship_gate_wait(
        &self,
        Parameters(args): Parameters<GateWaitArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::typed_result(
            self.use_cases
                .wait_gate(&args.gate_id, args.wait_secs)
                .await,
        )
    }

    #[tool(
        name = "ship_compact_receipts",
        description = "Permanently remove settled checks from older objectives. Keeps all current checks, unfinished checks, and checks needed for recovery. Requires an objective. Nothing is removed automatically. Returns removed and retained counts.",
        annotations(
            title = "Clear old check history",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn ship_compact_receipts(
        &self,
        Parameters(_): Parameters<NoArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::application_result(self.use_cases.compact_receipts())
    }
    #[tool(
        name = "ship_status",
        description = "Full state snapshot of the current execution cycle. Call this after context compaction to reconstruct where you are — objective, plan progress, active task, recent actions, pending checks, produced artifacts, and think_* cross-references.\n\nInputs: none.\n\nReturns: complete state including objective, tasks with status counts, active task details, recent actions, all checks, all artifacts.\n\nPitfalls: none — this is the recovery tool. Call it whenever you're unsure of the current state.",
        annotations(
            title = "Get status",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn ship_status(
        &self,
        Parameters(_): Parameters<NoArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::application_result(self.use_cases.status())
    }

    #[tool(
        name = "ship_export",
        description = "Export the full execution trace.\n\nInputs: format ('markdown'|'json', default 'markdown').\n\nReturns: the formatted trace.\n\nPitfalls: none.",
        annotations(
            title = "Export trace",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn ship_export(
        &self,
        Parameters(args): Parameters<ExportArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::application_result(
            self.use_cases
                .export(&args.format)
                .map(|trace| serde_json::json!({ "format": args.format, "trace": trace })),
        )
    }

    #[tool(
        name = "ship_reset",
        description = "Wipe all execution state — objective, tasks, actions, checks, artifacts. This is destructive and irreversible.\n\nInputs: none.\n\nReturns: confirmation.\n\nPitfalls: there is no undo.",
        annotations(
            title = "Reset (destructive)",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn ship_reset(
        &self,
        Parameters(_): Parameters<NoArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::application_result(
            self.use_cases
                .reset()
                .map(|()| serde_json::json!({ "status": "cleared" })),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn command_runner_captures_a_real_pass() {
        let out = run_check_command("exit 0").await;
        assert!(out.passed);
        assert_eq!(out.exit_code, Some(0));
    }

    #[tokio::test]
    async fn command_runner_captures_a_real_failure() {
        // A green check can't be faked: a failing command yields passed:false
        // and the real exit code, no matter what the agent claims. The fixture
        // speaks the platform shell, like run_check_command itself: `;` is not
        // a separator under `cmd /C`.
        #[cfg(windows)]
        let cmd = "echo boom 1>&2 & exit 7";
        #[cfg(not(windows))]
        let cmd = "echo boom >&2; exit 7";
        let out = run_check_command(cmd).await;
        assert!(!out.passed);
        assert_eq!(out.exit_code, Some(7));
        assert!(out.output_tail.contains("boom"));
    }

    #[test]
    fn tail_chars_keeps_the_end_and_marks_elision() {
        let s: String = std::iter::repeat_n('a', CHECK_OUTPUT_TAIL + 50).collect();
        let tailed = tail_chars(&s);
        assert!(tailed.contains("truncated"));
        assert!(tailed.ends_with('a'));
    }
}
