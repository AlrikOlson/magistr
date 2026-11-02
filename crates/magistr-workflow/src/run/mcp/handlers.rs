use rmcp::{
    ErrorData, handler::server::wrapper::Parameters, model::CallToolResult, tool, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;

use super::service::RunService;
use super::step_result::StepResult;
use crate::run::domain::artifact::{Artifact, ArtifactType};
use crate::run::domain::check::CheckType;
use crate::run::domain::goal::VerificationMode;
use crate::run::domain::progress::ProgressType;
use crate::run::domain::step::StepType;

impl RunService {
    pub(crate) fn make_tool_router() -> rmcp::handler::server::router::tool::ToolRouter<Self> {
        Self::tool_router()
    }
}

// ── Arg types ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RunOpenArgs {
    #[serde(default)]
    #[schemars(required, !default)]
    pub description: String,
    #[serde(default, deserialize_with = "crate::infra::coerce::string_or_seq")]
    pub acceptance_criteria: Vec<String>,
    #[serde(default, deserialize_with = "crate::infra::coerce::string_or_seq")]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub scope: String,
    /// `focused` (the default) requires the recorded acceptance checks.
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
    pub step_id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub step_type: Option<StepType>,
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
    /// error naming the valid progress.
    #[default]
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct StartArgs {
    #[serde(default)]
    #[schemars(required, !default)]
    pub step_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RecordArgs {
    #[serde(default)]
    pub step_id: Option<String>,
    #[serde(rename = "type", default = "default_progress_type")]
    pub progress_type: ProgressType,
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

fn default_progress_type() -> ProgressType {
    ProgressType::Code
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CompleteArgs {
    pub step_id: String,
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
    pub step_id: String,
    pub reason: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CheckArgs {
    #[serde(flatten)]
    pub options: crate::run::application::CheckOptions,
    #[serde(default)]
    pub step_id: Option<String>,
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
pub struct RunArgs {
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
pub struct ApprovalOpenArgs {
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
pub struct ApprovalWaitArgs {
    #[serde(default)]
    #[schemars(required, !default)]
    pub approval_id: String,
    /// Seconds this ONE call may wait before returning `pending` (default 25,
    /// max 55 — always under the MCP client's own call timeout; loop to keep
    /// waiting).
    #[serde(default)]
    pub wait_secs: Option<i64>,
}

/// The command execution budget, shared with MCP step retention.
pub const CHECK_TIMEOUT_SECS: u64 = crate::run::application::command_runner::CHECK_TIMEOUT_SECS;

#[cfg(test)]
use crate::run::application::command_runner::{CHECK_OUTPUT_TAIL, run_check_command, tail_chars};

// ── Tool handlers ──────────────────────────────────────────────────

#[tool_router]
impl RunService {
    #[tool(
        name = "run_open",
        description = "Open a run with its goal before arranging steps. Choose focused verification for bounded changes, including code: all recorded acceptance checks must pass, while unrelated project gates are omitted. Focused is the default. Use full only for an explicit full-suite request or concrete cross-cutting risk. Updating the same scope preserves its steps. Choosing a different scope saves the unfinished goal and plan; returning to a saved scope restores its goal details, verification mode, steps and checks unchanged. Supplied details seed a new scope; read run_status after selecting a scope. An unrelated unfinished goal is not a blocker and must not be reset or finalized to make room. Replacing a completed or abandoned goal retains its full records in work history and starts a new run, reporting retained_steps. Criteria and constraints accept arrays or a single string.",
        annotations(
            title = "Set goal",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn run_open(
        &self,
        Parameters(args): Parameters<RunOpenArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::typed_result(self.use_cases.set_goal_with_verification(
            args.description,
            args.acceptance_criteria,
            args.constraints,
            args.scope,
            args.verification,
        ))
    }

    #[tool(
        name = "arrange_steps",
        description = "Add, remove, or reorder the steps of the open run.\n\nInputs: action ('add'|'remove'|'reorder'), step_id (required), title (required for add), step_type ('implement'|'test'|'review'|'config'|'docs'|'research'), estimate ('trivial'|'small'|'medium'|'large'), after (step_id to place after, for reorder), think_branch (optional cross-ref to a think_* branch).\n\nReturns: the updated step list.\n\nPitfalls: step_id must be unique within the run for 'add'. Cannot remove an active or done step.",
        annotations(
            title = "Plan steps",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false,
        )
    )]
    pub async fn arrange_steps(
        &self,
        Parameters(args): Parameters<PlanArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        use crate::run::application::PlanChange;
        let change = match args.action {
            PlanAction::Add => PlanChange::Add {
                id: args.step_id,
                title: args.title,
                step_type: args.step_type,
                estimate: args.estimate,
                think_branch: args.think_branch,
            },
            PlanAction::Remove => PlanChange::Remove { id: args.step_id },
            PlanAction::Reorder => PlanChange::Reorder {
                id: args.step_id,
                after: args.after,
            },
            PlanAction::Unknown => PlanChange::Unknown,
        };
        Self::typed_result(self.use_cases.change_plan(change))
    }

    #[tool(
        name = "start_step",
        description = "Begin work on a step. Sets its status to active and records the start time. Only one step can be active at a time.\n\nInputs: step_id (required).\n\nReturns: the started step.\n\nPitfalls: fails if another step is already active. Finish the current step or mark it waiting first.",
        annotations(
            title = "Start step",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false,
        )
    )]
    pub async fn start_step(
        &self,
        Parameters(args): Parameters<StartArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::typed_result(
            self.use_cases
                .start_step(&args.step_id)
                .map(|observed| observed.map(StepResult::from)),
        )
    }

    #[tool(
        name = "record_progress",
        description = "Record progress on the active step. This is the primary workhorse — call it every time you do something: write code, run a command, make a decision, research something.\n\nInputs: step_id (optional — defaults to active step), type ('code'|'test'|'debug'|'research'|'config'|'refactor'|'review'), description (required), files_touched (string[]), tools_used (string[]), result (string), think_step (optional u32 — cross-ref to the think_* step that motivated this progress).\n\nReturns: the recorded progress with its assigned id.\n\nPitfalls: if no step is active and no step_id is provided, the call fails.",
        annotations(
            title = "Record progress",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false,
        )
    )]
    pub async fn record_progress(
        &self,
        Parameters(args): Parameters<RecordArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::typed_result(self.use_cases.record_progress(
            args.step_id.as_deref(),
            args.progress_type,
            args.description,
            args.files_touched,
            args.tools_used,
            args.result,
            args.think_step,
        ))
    }

    #[tool(
        name = "finish_step",
        description = "Close a step and record what was produced.\n\nInputs: step_id (required), artifacts (array of {type, ref, description}).\n\nReturns: the finished step.\n\nPitfalls: can only finish a step that is active or waiting.",
        annotations(
            title = "Complete step",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false,
        )
    )]
    pub async fn finish_step(
        &self,
        Parameters(args): Parameters<CompleteArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let artifacts = args.artifacts.into_iter().map(Artifact::from).collect();
        Self::typed_result(
            self.use_cases
                .complete_step(&args.step_id, artifacts)
                .map(|observed| observed.map(StepResult::from)),
        )
    }

    #[tool(
        name = "run_block_step",
        description = "Mark a step as waiting, with what it waits on.\n\nInputs: step_id (required), reason (required).\n\nReturns: the waiting step.\n\nPitfalls: only an active step can wait.",
        annotations(
            title = "Block step",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false,
        )
    )]
    pub async fn run_block_step(
        &self,
        Parameters(args): Parameters<BlockArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::typed_result(
            self.use_cases
                .block_step(&args.step_id, args.reason)
                .map(|observed| observed.map(StepResult::from)),
        )
    }

    #[tool(
        name = "check_step",
        description = "Run and record a check on step_id (defaults to the active step). PREFER passing `command`: the server runs it and records the real exit code, details and verified:true. Without command, passed is required and remains self-reported; use this only for checks that cannot run as commands. Optional report {format:'junit', path} parses a runner-produced report into counts and failing test names. A missing or malformed report does not change pass/fail or verified; the response reports the parsing error. Request a report from the runner instead of scraping stdout. Returns the recorded check and report/results when requested. Failed required checks and self-reported passes are flagged when the run finishes.",
        annotations(
            title = "Record check",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false,
        )
    )]
    pub async fn check_step(
        &self,
        Parameters(args): Parameters<CheckArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        use crate::run::application::{CheckRequest, RunUseCases};
        let request = CheckRequest {
            options: args.options,
            step_id: args.step_id,
            check_type: args.check_type,
            name: args.name,
            passed: args.passed,
            details: args.details,
            required: args.required,
            command: args.command,
            report: args.report.map(|report| (report.format, report.path)),
        };
        let admission = match self.use_cases.admit_check(request) {
            Ok(admission) => admission.value,
            Err(error) => return Self::typed_result::<()>(Err(error)),
        };
        let outcome = match self.use_cases.reuse_check(&admission) {
            Ok(Some(outcome)) => outcome,
            Ok(None) => RunUseCases::execute_check(&admission).await,
            Err(error) => return Self::typed_result::<()>(Err(error)),
        };
        Self::typed_result(
            self.use_cases
                .settle_check(admission, outcome)
                .map(super::check_step_output::CheckStepOutput::from),
        )
    }

    #[tool(
        name = "run_finish",
        description = "Finish the run and record its final artifacts (commits, PRs, deployments). Reviews all checks and warns about any required checks that failed or are missing.\n\nInputs: artifacts (array of {type, ref, description}), summary (optional string).\n\nReturns: run report — run status, step completion counts, check summary, warnings about failed/missing required checks.\n\nPitfalls: does NOT block on failed checks — it warns. The run history records whether the run finished with failures.",
        annotations(
            title = "Run goal",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false,
        )
    )]
    pub async fn run_finish(
        &self,
        Parameters(args): Parameters<RunArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let artifacts = args.artifacts.into_iter().map(Artifact::from).collect();
        Self::typed_result(self.use_cases.finalize(artifacts, args.summary))
    }

    #[tool(
        name = "approval_open",
        description = "Ask a person for approval: pause-worthy work that needs a human yes, answered from the workspace webapp. Creates a run/approval record in the cloud workspace and returns immediately with the approval id — then poll approval_wait. HEADLESS-SAFE BY CONSTRUCTION: `default` is required, the approval answers itself with that default at `timeout_secs`, and with no cloud workspace connected the call resolves to the default at once instead of opening anything.\n\nInputs: question (required — one plain sentence), body (plain-prose context), options (array of {key,label}; omitted = yes/no), default (required — the option key an unanswered gate resolves to), timeout_secs (default 3600, clamped 30..604800).\n\nReturns: {opened:true, approval_id, expires_at, ...} or {opened:false, resolved:<default>, reason} when no workspace could carry the gate.\n\nPitfalls: pick the SAFE default (usually 'no'/'hold') — it is what headless expiry applies.",
        annotations(
            title = "Open approval gate",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true,
        )
    )]
    pub async fn approval_open(
        &self,
        Parameters(args): Parameters<ApprovalOpenArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        use crate::run::application::ApprovalRequest;
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
                .open_approval(ApprovalRequest {
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
        name = "approval_wait",
        description = "Check (and briefly wait on) an open approval. Polls the cloud workspace; returns as soon as the approval is answered or expired, else after wait_secs with state 'pending' — loop on pending to keep waiting. The wait is BOUNDED (max 55s per call, under any MCP client timeout); an unanswered approval deterministically resolves to its declared default once expires_at passes, so a headless session always completes.\n\nInputs: approval_id (required — from approval_open), wait_secs (default 25, clamped 0..55).\n\nReturns: {state:'answered', choice, decided_by, decided_at, note?} | {state:'expired', choice:<default>} | {state:'pending', seconds_left}.\n\nPitfalls: if the workspace is unreachable, this returns a soft error — apply your declared default yourself once expires_at passes.",
        annotations(
            title = "Wait on approval gate",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = true,
        )
    )]
    pub async fn approval_wait(
        &self,
        Parameters(args): Parameters<ApprovalWaitArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::typed_result(
            self.use_cases
                .wait_approval(&args.approval_id, args.wait_secs)
                .await,
        )
    }

    #[tool(
        name = "run_compact_checks",
        description = "Permanently remove settled check results from older runs. Keeps all current checks, unfinished checks, and checks needed for recovery. Requires an open run. Nothing is removed automatically. Returns removed and retained counts.",
        annotations(
            title = "Clear old check history",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn run_compact_checks(
        &self,
        Parameters(_): Parameters<NoArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::typed_result(self.use_cases.compact_check_results())
    }
    #[tool(
        name = "run_status",
        description = "Full state snapshot of the current run. Call this after context compaction to reconstruct where you are — goal, plan progress, active step, recent progress, pending checks, produced artifacts, and think_* cross-references.\n\nInputs: none.\n\nReturns: complete state including goal, steps with status counts, active step details, recent progress, all checks, all artifacts.\n\nPitfalls: none — this is the recovery tool. Call it whenever you're unsure of the current state.",
        annotations(
            title = "Get status",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn run_status(
        &self,
        Parameters(_): Parameters<NoArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::typed_result(self.use_cases.status())
    }

    #[tool(
        name = "run_export",
        description = "Export the full run history.\n\nInputs: format ('markdown'|'json', default 'markdown').\n\nReturns: the formatted run history.\n\nPitfalls: none.",
        annotations(
            title = "Export trace",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn run_export(
        &self,
        Parameters(args): Parameters<ExportArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::typed_result(
            self.use_cases
                .export(&args.format)
                .map(|trace| serde_json::json!({ "format": args.format, "trace": trace })),
        )
    }

    #[tool(
        name = "run_reset",
        description = "Wipe every run — goal, steps, progress, checks, artifacts. This is destructive and irreversible.\n\nInputs: none.\n\nReturns: confirmation.\n\nPitfalls: there is no undo.",
        annotations(
            title = "Reset (destructive)",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn run_reset(
        &self,
        Parameters(_): Parameters<NoArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::typed_result(
            self.use_cases
                .reset()
                .map(|observed| observed.map(|()| serde_json::json!({ "status": "cleared" }))),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn command_runner_captures_a_real_pass() {
        let out = run_check_command("exit 0", None).await;
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
        let out = run_check_command(cmd, None).await;
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
