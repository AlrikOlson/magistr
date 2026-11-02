use rmcp::{
    ErrorData, handler::server::wrapper::Parameters, model::CallToolResult, tool, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;

use super::service::PlanService;
use crate::infra::CrossRef;
use crate::plan::application::{
    ExportRequest, FeatureView, PlanError, TaskDraft, TaskPatch, WaitingDraft,
};
use crate::plan::domain::TaskStatus;

impl PlanService {
    pub(crate) fn make_tool_router() -> rmcp::handler::server::router::tool::ToolRouter<Self> {
        Self::tool_router()
    }
}

/// Parse a wire status string into the domain enum (snake_case), surfacing a
/// readable error for the structured-error envelope.
fn parse_status(s: &str) -> Result<TaskStatus, String> {
    match s {
        "idea" => Ok(TaskStatus::Idea),
        "to_do" => Ok(TaskStatus::ToDo),
        "in_progress" => Ok(TaskStatus::InProgress),
        "waiting" => Ok(TaskStatus::Waiting),
        "done" => Ok(TaskStatus::Done),
        "retired" => Ok(TaskStatus::Retired),
        _ => Err(format!(
            "invalid status '{s}' (expected idea|to_do|in_progress|waiting|done|retired)"
        )),
    }
}

// Priority parsing lives in `infra::coerce` (the single, infallible home):
// `coerce::priority` accepts int | numeric string | named level and falls
// back to 0 rather than erroring, so it can never emit a -32602.

// ── Arg types ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddTaskArgs {
    #[serde(default)]
    #[schemars(required, !default)]
    pub id: String,
    #[serde(default)]
    #[schemars(required, !default)]
    pub title: String,
    // The full rule lives in the tool description and in the engine's refusal,
    // which are the two copies a caller actually reads — one before forming the
    // call, one when they get it wrong. A third copy here only costs bytes in
    // every tools/list handshake.
    /// Short node label, max 24 chars. Derived from the id if omitted.
    #[serde(default)]
    pub name: String,
    /// Initial status (default `to_do`).
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default, deserialize_with = "crate::infra::coerce::priority")]
    pub priority: u32,
    #[serde(default)]
    pub description: String,
    #[serde(default, deserialize_with = "crate::infra::coerce::string_or_seq")]
    pub acceptance: Vec<String>,
    #[serde(default, deserialize_with = "crate::infra::coerce::string_or_seq")]
    pub deps: Vec<String>,
    #[serde(default)]
    pub shared: bool,
    /// The feature. Omit for unassigned.
    #[serde(default)]
    pub feature: Option<String>,
    /// PREFER THIS over packing detail into `description` — the structured
    /// body that renders as UI: {version:1, summary: one plain sentence (no
    /// shorthand, no ids), facts?: [{label,value}], sections?: [{heading,
    /// prose?: markdown, list?: [{text, done?}]}]}. `description` stays the
    /// plain-prose fallback.
    #[serde(default)]
    #[schemars(schema_with = "crate::content::input_schema")]
    pub content: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TrackerSetupArgs {
    /// Which tracker: `linear` or `github`.
    #[serde(default = "default_provider")]
    pub provider: String,
    /// A Linear team KEY (letters and digits only, e.g. `ENG`) or a GitHub
    /// `owner/repo`.
    #[serde(default)]
    #[schemars(required, !default)]
    pub into: String,
    /// Display name to use if the destination has to be created.
    #[serde(default)]
    pub name: Option<String>,
    /// Only include one priority band: critical|high|medium|low|later.
    #[serde(default)]
    pub band: Option<String>,
    /// Name for the initiative (the roof mirrored projects file under), for
    /// providers that have one. Absent keeps the configured name or the
    /// directory-basename default; blank is a no-op.
    #[serde(default)]
    pub initiative: Option<String>,
    /// Create the destination upstream when it is absent. **Defaults to false.**
    /// This writes to a third-party system and cannot be undone from here — only
    /// set it when the human has asked for the destination to be created.
    #[serde(default)]
    pub create_missing: bool,
    /// Report what would happen and write nothing, locally or upstream.
    #[serde(default)]
    pub dry_run: bool,
    /// RETIRED, and deliberately absent from the advertised schema. The
    /// standalone server's push cadence read this; Magistr runs none. It is
    /// still deserialized so a caller following an older description is
    /// refused by name instead of having the setting silently ignored.
    #[serde(default)]
    #[schemars(skip)]
    pub push_secs: Option<u64>,
}

fn default_provider() -> String {
    "github".to_string()
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetStatusArgs {
    #[serde(default)]
    #[schemars(required, !default)]
    pub id: String,
    /// One of: ideas, pending, in_progress, blocked, done, retired.
    #[serde(default)]
    #[schemars(required, !default)]
    pub status: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateTaskArgs {
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    /// Rewrite the short node label (at most 24 characters), leaving the
    /// sentence `title` alone. Send an empty string to re-seed it from the id.
    #[serde(default)]
    pub name: Option<String>,
    /// Accepts a number or a named level, like `plan_add_task` — it took
    /// only numbers, so the same word worked on one tool and not the other.
    #[serde(default, deserialize_with = "crate::infra::coerce::optional_priority")]
    pub priority: Option<u32>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub acceptance: Option<Vec<String>>,
    #[serde(default)]
    pub deps: Option<Vec<String>>,
    /// Replace the task's structured body (same shape as
    /// `plan_add_task.content`). Absent leaves stored content unchanged.
    #[serde(default)]
    #[schemars(schema_with = "crate::content::input_schema")]
    pub content: Option<serde_json::Value>,
    /// Resolve an open title proposal: "accept" adopts the tracker's title
    /// into the plan, "reject" clears the proposal so the plan's title flows
    /// again. Errors when the task has no open proposal.
    #[serde(default)]
    pub resolve_title_proposal: Option<String>,
    /// Why this task cannot be worked, when the answer is not another task
    /// (that is `deps`).
    #[serde(default)]
    pub waiting_on: Option<WaitingOnArgs>,
    /// Retract this task's waiting reason. Errors when there is none to retract.
    #[serde(default, deserialize_with = "crate::infra::coerce::lenient_bool")]
    pub clear_waiting_on: bool,
}

// The waiting reason payload on `UpdateTaskArgs`.
//
// DELIBERATELY NOT A DOC COMMENT. schemars serializes a `///` on a struct into
// the JSON schema's `description`, and this input schema is charged to
// `model_facing_tool_surface_stays_lean` — the bytes every agent pays on every
// session. Written as `///` first, this rationale alone cost ~520 B of context
// to explain an internal serde decision no caller acts on. Rationale for the
// maintainer goes in `//`; only what steers a CALLER earns a `///` here.
//
// Both strings are `#[serde(default)]` and neither is `required`, for the
// reason `add_task_priority_named_or_numeric` records: a missing field must
// reach the handler as an empty string and come back as a readable soft error,
// because a hard `-32602` from deserialization cancels every sibling tool call
// in a parallel batch. The emptiness checks live in
// `PlanEngine::validate_waiting_on`, which already refuses a blank reason.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct WaitingOnArgs {
    /// disproved | condition | person | external.
    #[serde(default)]
    #[schemars(required, !default)]
    pub kind: String,
    /// Why, in a sentence nobody should have to re-derive from the title.
    #[serde(default)]
    #[schemars(required, !default)]
    pub reason: String,
    /// Optional proof — a cross-ref (decision:N | task:id | step:X).
    #[serde(default)]
    pub evidence: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ObsoleteArgs {
    pub id: String,
    pub reason: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReprioritizeArgs {
    #[serde(default)]
    #[schemars(required, !default)]
    pub id: String,
    #[serde(default, deserialize_with = "crate::infra::coerce::priority")]
    pub suggested_priority: u32,
    #[serde(default)]
    #[schemars(required, !default)]
    pub reason: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct ExportArgs {
    #[serde(default = "default_format", deserialize_with = "export_string")]
    pub format: String,
    #[serde(default = "default_output", deserialize_with = "export_string")]
    pub output: String,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

// Missing keys receive defaults; explicit null/wrong types reach the handler's
// structured invalid_args envelope instead of cancelling sibling tool calls.
fn export_string<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    Ok(serde_json::Value::deserialize(deserializer)?
        .as_str()
        .unwrap_or_default()
        .to_string())
}

fn default_output() -> String {
    "file".to_string()
}

fn default_format() -> String {
    "markdown".to_string()
}

// The bounded-fetch arguments.
//
// DELIBERATELY LIGHT ON `///`. schemars serializes every doc comment here into
// the input schema, which is charged to `model_facing_tool_surface_stays_lean`
// — bytes every agent pays on every session. The cap, the vocabulary and the
// three refusals are explained once, in `PlanEngine::get`, where a
// maintainer reads them; only what a CALLER must know to form a call is here.
//
// Both fields use `string_or_seq` for the reason `add_task_priority_named_or_numeric`
// records: a wrong shape must reach the handler and come back as a readable
// soft error, because a hard `-32602` from deserialization cancels every
// sibling tool call in a parallel batch. It also lets a single id arrive as a
// bare string.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetArgs {
    /// The task ids to fetch.
    #[serde(default, deserialize_with = "crate::infra::coerce::string_or_seq")]
    pub ids: Vec<String>,
    /// Project each record to these fields. Omit for the whole record.
    #[serde(default, deserialize_with = "crate::infra::coerce::string_or_seq")]
    pub fields: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct StartTaskArgs {
    pub id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CompleteTaskArgs {
    pub id: String,
    /// Optional proof-of-run cross-ref to attach (e.g. `step:<id>`,
    /// `check:<name>`).
    #[serde(default)]
    pub step_ref: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LinkArgs {
    #[serde(default)]
    #[schemars(required, !default)]
    pub id: String,
    /// A reference: `decision:N`, `step:X`, `progress:N`, `check:X`, `task:X`.
    #[serde(default, rename = "ref")]
    #[schemars(required, !default)]
    pub reference: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RecordRefreshArgs {
    pub summary: String,
    /// decision note ids that motivated the mutation.
    #[serde(default)]
    pub notes: Vec<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetFeatureArgs {
    #[serde(default)]
    #[schemars(required, !default)]
    pub id: String,
    /// The feature name. Omit or pass empty to CLEAR the feature — unassigned
    /// is a valid answer, not a gap.
    #[serde(default)]
    pub feature: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProposeFeaturesArgs {
    /// Only report a cluster of at least this many unassigned tasks. The floor
    /// is the point: without it a plan proposes a container per one-off slug.
    #[serde(default = "default_min_shared")]
    pub min_shared: usize,
}

const fn default_min_shared() -> usize {
    4
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct NoArgs {}

// Field docs here are deliberately terse. Every `///` below is serialized into
// the tool's inputSchema and shipped in tools/list on every handshake, and the
// team recipe in particular was written three times over — in the description
// the model reads, in the schema, and in the refusal a caller actually hits
// when they get it wrong. Two of those three are the ones that reach a person;
// the schema is the copy that only costs bytes.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct FocusGetArgs {
    /// Required. The asking caller's stable identity.
    #[serde(default)]
    #[schemars(required, !default)]
    pub team: String,
    /// Inspect this feature without changing focus.
    #[serde(default)]
    pub feature: Option<String>,
    /// current (default) or history.
    #[serde(default)]
    pub view: FeatureView,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FocusSetArgs {
    /// Required. The focusing caller's stable identity.
    #[serde(default)]
    #[schemars(required, !default)]
    pub team: String,
    /// Current feature name, or an unambiguous current fragment.
    #[serde(default)]
    pub feature: String,
    /// shape | build | listen.
    #[serde(default)]
    pub mode: String,
    /// Release this team's focus; `feature` and `mode` are then ignored.
    #[serde(default)]
    pub clear: bool,
}

fn native_patch(args: UpdateTaskArgs) -> Result<(String, TaskPatch), PlanError> {
    let content = crate::content::parse_optional(args.content).map_err(PlanError::InvalidArgs)?;
    let resolve_title_proposal = args
        .resolve_title_proposal
        .as_deref()
        .map(|verb| match verb.trim().to_ascii_lowercase().as_str() {
            "accept" => Ok(true),
            "reject" => Ok(false),
            other => Err(PlanError::InvalidState(format!(
                "resolve_title_proposal must be 'accept' or 'reject', got '{other}'"
            ))),
        })
        .transpose()?;
    let waiting_on = args
        .waiting_on
        .map(|waiting| {
            crate::plan::domain::WaitingReason::from_wire(&waiting.kind)
                .map(|kind| WaitingDraft {
                    kind,
                    reason: waiting.reason,
                    evidence: waiting.evidence,
                })
                .map_err(PlanError::InvalidArgs)
        })
        .transpose()?;
    Ok((
        args.id,
        TaskPatch {
            title: args.title,
            name: args.name,
            priority: args.priority,
            description: args.description,
            acceptance: args.acceptance,
            deps: args.deps,
            content,
            resolve_title_proposal,
            waiting_on,
            clear_waiting_on: args.clear_waiting_on,
        },
    ))
}

// ── Tool handlers ──────────────────────────────────────────────────

#[tool_router]
impl PlanService {
    #[tool(
        name = "plan_add_task",
        description = "Add a task (phase/item) to the plan.\n\nInputs: id (required, stable slug), title (required — the full claim, a sentence is fine), name (the short label a canvas node wears, max 24 chars, e.g. 'Quota ceiling'; omit it and one is derived from the id, send a sentence and the call is REJECTED), status ('idea'|'to_do'|'in_progress'|'waiting'|'done'|'retired', default 'to_do'), priority (integer OR a named level 'critical'|'high'|'medium'|'low' → 100/200/300/400; lower sorts earlier), description, acceptance (string[]), deps (string[] of task ids), shared (bool — committed vs gitignored partition), feature (the feature — PASS IT AT BIRTH; an unassigned task is invisible to every focused read), content (RECOMMENDED — structured body: {version:1, summary, facts?[{label,value}], sections?[{heading, prose?, list?[{text,done?}]}]}; write the detail here so the webapp renders it as UI, and keep description to a short plain-prose fallback).\n\nReturns: the created task.\n\nPitfalls: id must be unique; malformed content is rejected with the exact field named; an unusable feature refuses the whole call rather than creating an unassigned task. plan_* is the long-horizon plan above run_* goals.",
        annotations(
            title = "Add plan task",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false,
        )
    )]
    pub async fn plan_add_task(
        &self,
        Parameters(args): Parameters<AddTaskArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let status = match args.status.as_deref().map(parse_status).transpose() {
            Ok(status) => status.unwrap_or(TaskStatus::ToDo),
            Err(error) => return Ok(Self::err_structured("invalid_args", error)),
        };
        let content = match crate::content::parse_optional(args.content) {
            Ok(content) => content,
            Err(error) => return Ok(Self::err_structured("invalid_args", error)),
        };
        Self::present(self.application.add_task(TaskDraft {
            id: args.id,
            title: args.title,
            name: args.name,
            status,
            priority: args.priority,
            description: args.description,
            acceptance: args.acceptance,
            deps: args.deps,
            shared: args.shared,
            feature: args.feature,
            content,
        }))
    }

    #[tool(
        name = "plan_set_status",
        description = "Transition a task to a new status, validated against the lifecycle table.\n\nInputs: id (required), status (required — idea|to_do|in_progress|waiting|done|retired).\n\nReturns: the updated task.\n\nAn explicit transition also disposes any open status proposal on the task: transitioning to the suggested status accepts it, transitioning anywhere else supersedes it. The sweep re-proposes if the divergence persists.\n\nPitfalls: illegal transitions (e.g. done→retired) are rejected",
        annotations(
            title = "Set task status",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false,
        )
    )]
    pub async fn plan_set_status(
        &self,
        Parameters(args): Parameters<SetStatusArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        match parse_status(&args.status) {
            Ok(status) => Self::present(self.application.set_status(&args.id, status)),
            Err(error) => Ok(Self::err_structured("invalid_args", error)),
        }
    }

    #[tool(
        name = "plan_update_task",
        description = "Patch a task's descriptive fields (status is handled by plan_set_status).\n\nInputs: id (required); any of title, name (the short node label, max 24 chars — send an empty string to re-seed it from the id), priority, description, acceptance (string[]), deps (string[]), content — omitted fields are left unchanged. content (RECOMMENDED when a task carries real detail) replaces the structured body, same shape as plan_add_task.content — the shape the webapp renders as UI; keep description as the short plain-prose fallback. resolve_title_proposal ('accept'|'reject') resolves an open contested-title proposal: accept adopts the tracker's title into the plan, reject clears it so the plan's title flows again.\n\nwaiting_on records why a task cannot be worked when the answer is NOT another task — a refuted premise, a wait on a person, an external dependency — so it stops living as prose in the title. clear_waiting_on:true retracts it, and retracting is meant to be as easy as recording: a waiting reason nobody clears rots into the stale prose the field replaced. Sending both is refused.\n\nReturns: the updated task.\n\nAn explicit differing edit of a proposed-about field disposes that field's open proposal: a title edit disposes the title proposal, a priority edit disposes the reprioritize proposal.",
        annotations(
            title = "Update task",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn plan_update_task(
        &self,
        Parameters(args): Parameters<UpdateTaskArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        match native_patch(args) {
            Ok((id, patch)) => Self::present(self.application.update_task(&id, patch)),
            Err(error) => Self::present::<()>(Err(error)),
        }
    }

    #[tool(
        name = "plan_retire_task",
        description = "Retire a task with a reason. Kept for history, never selected by plan_next.\n\nInputs: id (required), reason (required).\n\nReturns: the retired task.",
        annotations(
            title = "Obsolete task",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn plan_retire_task(
        &self,
        Parameters(args): Parameters<ObsoleteArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::present(self.application.retire_task(&args.id, args.reason))
    }

    #[tool(
        name = "plan_reprioritize",
        description = "Record a re-prioritization PROPOSAL. This does NOT reorder the plan — it attaches a suggested priority + reason for a human to accept. Re-prioritization stays a human decision.\n\nInputs: id (required), suggested_priority (integer OR a named level 'critical'|'high'|'medium'|'low', required), reason (required).\n\nReturns: the task with the pending proposal attached (its real priority is unchanged).\n\nIdempotent for an unchanged suggestion. Disposed by an explicit priority edit via plan_update_task (accepting = setting the suggested priority).",
        annotations(
            title = "Propose re-prioritization",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn plan_reprioritize(
        &self,
        Parameters(args): Parameters<ReprioritizeArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::present(self.application.propose_reprioritize(
            &args.id,
            args.suggested_priority,
            args.reason,
        ))
    }

    #[tool(
        name = "plan_next",
        description = "Return the next ready task: the MOST URGENT `to_do` task — the one with the smallest `priority` number, since lower sorts earlier (critical=100, high=200, medium=300, low=400) — that carries no `waiting_on` and whose dependencies are all `done`. Returns null when nothing is ready.\n\nInputs: none.\n\nReturns: the task, or null.",
        annotations(
            title = "Next ready task",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn plan_next(
        &self,
        Parameters(_): Parameters<NoArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::present(self.application.next())
    }

    #[tool(
        name = "plan_status",
        description = "Plan snapshot: status counts, the next-ready task id, and the priority-sorted task list.\n\nInputs: none.\n\nReturns: counts + next + tasks. Call this to reconstruct the plan after context loss.",
        annotations(
            title = "Plan status",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn plan_status(
        &self,
        Parameters(_): Parameters<NoArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::present(self.application.status())
    }

    #[tool(
        name = "plan_export",
        description = "Atomically write PLAN.md (or PLAN.json) in the configured workspace; no manual copying.\n\nInputs: format ('markdown'|'json', default 'markdown'); output ('file'|'inline', default 'file').\n\nReturns: {format, path, bytes, written:true}. output:'inline' returns {format,plan} without writing. Use plan_get for selected records. Refuses symlink/non-file targets and unknown inputs.",
        annotations(
            title = "Export plan",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn plan_export(
        &self,
        Parameters(args): Parameters<ExportArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        if !args.extra.is_empty() {
            return Ok(Self::err_structured(
                "invalid_args",
                "only format and output are supported",
            ));
        }
        let request = args.format.parse().and_then(|format| {
            args.output
                .parse()
                .map(|target| ExportRequest { format, target })
        });
        match request {
            Ok(request) => Self::present(self.application.export(request)),
            Err(error) => Self::present::<()>(Err(error)),
        }
    }

    #[tool(
        name = "plan_get",
        description = "Read full stored records — description, content, acceptance, deps, refs — for selected tasks. plan_status omits these bodies; plan_export writes the whole database.\n\nInputs: ids (string[], required, at most 20 DISTINCT ids; repeats are answered once); fields (string[], optional) projects each record, e.g. ['content'] or ['acceptance','deps'].\n\nReturns: { records, returned, unknown, fields }. Records come back in the order you named them.\n\nPitfalls: nothing here fails quietly. Over the cap is an ERROR, not a short answer; an id matching no task is listed in `unknown` rather than omitted, so you can tell 'no such task' from 'that task has no summary'; and an unrecognised field name is refused with the full vocabulary rather than ignored. `id` is returned whatever you project.",
        annotations(
            title = "Get tasks by id",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn plan_get(
        &self,
        Parameters(args): Parameters<GetArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::present(self.application.get(&args.ids, &args.fields))
    }

    #[tool(
        name = "plan_start_task",
        description = "Begin work on a task: marks it in_progress and returns the `task:<id>` reference. Wire that reference into run_open (as the goal's scope/ref) and into a decision note's run_ref, then plan_link the resulting task:/decision: refs back to close the loop.\n\nInputs: id (required).\n\nReturns: { task, backref, hint }.\n\nPitfalls: only to-do work with no waiting reason and all dependencies completed can start. The writer rechecks current records.",
        annotations(
            title = "Start task",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false,
        )
    )]
    pub async fn plan_start_task(
        &self,
        Parameters(args): Parameters<StartTaskArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::present(self.application.start_task(&args.id).map(|observed| observed.map(|task| serde_json::json!({
            "task": task, "ref": CrossRef::PlanTask(args.id).to_wire(),
            "hint": "pass `ref` to run_open and a decision run_ref, then plan_link the resulting task:/decision: refs back",
        }))))
    }

    #[tool(
        name = "plan_complete_task",
        description = "Mark a task done on the current run's evidence: the goal is scoped task:<id> and its verification task (step_ref step:<id>, else where the receipts landed) holds current passing verified receipts for every required gate.\n\nInputs: id (required), step_ref (optional).\n\nReturns: the completed task.\n\nPitfalls: a refusal names each outstanding gate; re-run and retry. Only InProgress tasks go done.",
        annotations(
            title = "Complete task",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false,
        )
    )]
    pub async fn plan_complete_task(
        &self,
        Parameters(args): Parameters<CompleteTaskArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::present(
            self.application
                .complete_task(&args.id, args.step_ref.as_deref()),
        )
    }

    #[tool(
        name = "plan_link",
        description = "Attach a reference to a task, fusing the three families into one graph. Validated against the reference wire format; duplicates are ignored.\n\nInputs: id (required), ref (required — decision:N | step:X | progress:N | check:X | task:X).\n\nReturns: the updated task.",
        annotations(
            title = "Link task",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn plan_link(
        &self,
        Parameters(args): Parameters<LinkArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::present(self.application.link_task(&args.id, &args.reference))
    }

    #[tool(
        name = "plan_record_refresh",
        description = "Append a datestamped refresh note recording a plan mutation and the decision note ids that motivated it (the /roadmap-refresh provenance, made first-class).\n\nInputs: summary (required), notes (u32[]).\n\nReturns: { recorded, summary, notes, total_refreshes }.",
        annotations(
            title = "Record refresh note",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false,
        )
    )]
    pub async fn plan_record_refresh(
        &self,
        Parameters(args): Parameters<RecordRefreshArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .application
            .record_refresh(args.summary.clone(), args.notes.clone())
        {
            Ok(total) => Self::present(Ok(total.map(|total| {
                serde_json::json!({
                    "recorded": true, "summary": args.summary, "notes": args.notes,
                    "total_refreshes": total,
                })
            }))),
            Err(error) => Self::present::<()>(Err(error)),
        }
    }

    #[tool(
        name = "plan_set_feature",
        description = "Put a task in a feature, or take it out of one. The feature is what a tracker maps to a container — a Linear PROJECT.\n\nInputs: id (required), feature (the feature name; omit or pass empty to CLEAR it).\n\nThe feature is also the task's REGION on the tech-tree canvas, and the two readings pull different ways. A tracker is happy with no feature: such a task mirrors as a plain issue with no project, and a container holding one item is worse than no container. A canvas is not: a task with no region is drawn in 'Uncharted ground', the one region for what nobody has placed. Prefer an existing region over a new one — the map level can only carry 20 marks.\n\nA name that repeats a task id prefix is REFUSED here and now, not reported by `doctor` later: 'signal' or 'saas' is a slug wearing a region's job, and a region has to be somewhere a person could point at. Name the place, not the prefix.\n\nReturns: the updated task, or an invalid_state error naming the slug it clashed with.",
        annotations(
            title = "Set a task's feature",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn plan_set_feature(
        &self,
        Parameters(args): Parameters<SetFeatureArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::present(self.application.set_feature(&args.id, args.feature))
    }

    #[tool(
        name = "plan_focus_get",
        description = "Read a team's exact saved focus and inspect current work or history. Never changes focus.\n\nInputs: team (required stable caller identity); feature (optional current name/unique fragment or exact historical name); view (current default | history). No feature inspects the saved name exactly.\n\nReturns: team, focus, focus_state (unset|current|closed|missing), frontier, features, persistent. frontier state: unset; current with frontier {feature,activity:{counts,readiness,next_ready},items}; closed with feature/history counts; history with feature/history {done,retired,task_ids}; missing with query; ambiguous with query/matches. features holds current features, history counts, unassigned activity and unassigned_history counts. Only view:history includes historical IDs, for mixed or closed features.\n\nPitfalls: peeks leave focus_state unchanged. Closed/missing saved names never redirect. Unknown teams return unset plus catalog. persistent says whether focus survives restart.",
        annotations(
            title = "Read a team's focus",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn plan_focus_get(
        &self,
        Parameters(args): Parameters<FocusGetArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::present(
            self.application
                .focus_get(&args.team, args.feature.as_deref(), args.view),
        )
    }

    #[tool(
        name = "plan_focus_set",
        description = "Select a current feature and mode for one team, or release its focus. No task status, order or work changes.\n\nInputs: team (required stable caller identity), feature (current name/unique fragment), mode (shape|build|listen), clear (release; feature/mode ignored).\n\nReturns: team, focus, frontier {feature,activity:{counts,readiness,next_ready},items}, persistent. Clear returns focus:null and released.\n\nPitfalls: closed, unknown or ambiguous names write nothing and report current choices; prior focus survives. Blank teams are refused. Modes accept no synonyms; implement is not build.",
        annotations(
            title = "Set a team's focus",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn plan_focus_set(
        &self,
        Parameters(args): Parameters<FocusSetArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        if args.clear {
            return Self::present(self.application.focus_clear(&args.team));
        }
        match crate::plan::domain::FocusMode::from_wire(&args.mode) {
            Ok(mode) => Self::present(self.application.focus_set(&args.team, &args.feature, mode)),
            Err(error) => Ok(Self::err_structured("invalid_args", error)),
        }
    }

    #[tool(
        name = "plan_propose_features",
        description = "Find clusters of unfinished, unassigned tasks to name. Shared id prefixes require min_shared tasks (default 4).\n\nReturns: clusters (prefix, task IDs, naming restriction), min_shared, authored_names (current and historical names to avoid collisions), placed:0. authored_names is not current choices.\n\nPitfalls: this places nothing. Choose a feature name and apply it with plan_set_feature. Using the cluster's prefix as its name is refused.",
        annotations(
            title = "Propose features from task ids",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn plan_propose_features(
        &self,
        Parameters(args): Parameters<ProposeFeaturesArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::present(self.application.propose_features(args.min_shared))
    }

    #[tool(
        name = "tracker_status",
        description = "Where this project's plan is mirrored, if anywhere. Read-only.\n\nInputs: none.\n\nReturns: enabled, provider, target, included (tasks opted in), not_included (ACTIVE tasks that are not — the drift), and inherits_new_tasks (whether new tasks are born in scope). Call this before tracker_setup to see whether there is anything to do, and after it to confirm what changed.\n\nPitfalls: `included` only ever grows, so it cannot tell you the scope went stale — read `not_included`. A non-zero not_included with inherits_new_tasks false means work is invisible to the tracker.",
        annotations(
            title = "Tracker mirroring status",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn tracker_status(
        &self,
        Parameters(_): Parameters<NoArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let (_, _, config) = crate::tracker::host::tracker_config();
        Self::present(self.application.tracker_status(&config))
    }

    #[tool(
        name = "tracker_setup",
        description = "Connect this project's plan to an issue tracker: check the destination exists, turn mirroring on for this project, and opt the active tasks in.\n\nInputs: provider ('linear'|'github', default 'github'), into (REQUIRED — a Linear team key like ENG, or a GitHub owner/repo), name (display name if it has to be created), band (only include one priority band), initiative (name for the roof the mirrored projects file under; absent keeps the configured or directory-basename default), create_missing (default FALSE — see below), dry_run.\n\nCREATE_MISSING WRITES TO SOMEBODY ELSE'S SYSTEM. It creates the team/board upstream, which cannot be undone from here. Leave it false unless the human you are working with has actually asked for the destination to be created. With it false, a missing destination stops the run and nothing at all is written.\n\nNOTE Linear team keys are letters and digits only (ENG, WOW) — Linear silently rewrites anything else into a different key.\n\nReturns: what it did — target_existed/target_created/target_unverified, mirroring_enabled, included[], already_included, and any follow-up the human must do.\n\nNothing is sent by this tool, and Magistr runs no background push: opt-ins are plan state, and no cadence projects them. A push interval is refused, not scheduled; say so rather than promising a sync.",
        annotations(
            title = "Set up tracker mirroring",
            read_only_hint = false,
            // Both true and both meant: it can create a team upstream, and it
            // reaches a third-party system over the network.
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = true,
        )
    )]
    pub async fn tracker_setup(
        &self,
        Parameters(args): Parameters<TrackerSetupArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        if args.push_secs.is_some() {
            // Refused before the port is built or anything is written: the
            // setting names a cadence this host does not run, and accepting
            // it would be the silent promise this contract no longer makes.
            return Ok(crate::infra::tool_failure::ToolFailure::rejected(
                "unsupported_setting",
                "push_secs is not a setting of this host: Magistr runs no unattended \
                 tracker push, so no interval can be scheduled. Call again without it; \
                 opt-ins are recorded and nothing is sent.",
                vec![serde_json::json!({
                    "field": "push_secs",
                    "problem": "retired: no background push cadence exists on this host",
                })],
            ));
        }
        let (data_dir, _, _) = crate::tracker::host::tracker_config();
        let req = crate::plan::application::tracker::SetupRequest {
            provider: args.provider.clone(),
            into: args.into.clone(),
            name: args.name.clone(),
            band: args.band.clone(),
            initiative: args.initiative.clone(),
            // NOT wired to anything a caller can set. `yes` is the CLI's
            // "skip the prompt" flag and this path has no prompt to skip —
            // `create_missing` is the whole of the decision here.
            yes: false,
            dry_run: args.dry_run,
        };

        let probe_cfg = crate::tracker::TrackerConfig {
            enabled: true,
            provider: Some(req.provider.trim().to_ascii_lowercase()),
            target: Some(req.into.trim().to_string()),
            ..crate::tracker::TrackerConfig::default()
        };
        let port = match crate::tracker::host::tracker_port_async(&probe_cfg).await {
            Ok(p) => p,
            Err(e) => return Ok(Self::err_structured("invalid_target", e.to_string())),
        };

        let report = match self
            .application
            .setup_tracker(port.as_ref(), &req, args.create_missing, &data_dir)
            .await
        {
            Ok(report) => report,
            Err(error) => {
                use crate::infra::tool_failure::ToolFailure;
                return Ok(if error.effect == "unknown" {
                    ToolFailure::uncertain(error.kind, error.message)
                } else if let Some(store) = error.store {
                    ToolFailure::store(error.kind, &store)
                } else {
                    Self::err_structured(error.kind, error.message)
                });
            }
        };
        let (project_id, phase, outcome) = (report.project_id, report.phase, report.outcome);
        use crate::plan::application::tracker::TargetPhase;

        let mut next_steps: Vec<String> = Vec::new();
        if !outcome.included.is_empty() {
            next_steps.push(
                "Nothing has been sent: the opt-ins are recorded as plan \
                 state, and Magistr runs no background push that would project \
                 them."
                    .into(),
            );
        }

        Ok(Self::ok_structured(serde_json::json!({
            "project_id": project_id,
            "provider": req.provider,
            "target": req.into,
            "dry_run": req.dry_run,
            "target_existed": phase == TargetPhase::Present,
            "target_created": phase == TargetPhase::Created,
            "target_unverified": phase == TargetPhase::Unverifiable,
            "mirroring_enabled": outcome.mirroring_enabled,
            "included": outcome.included,
            "already_included": outcome.already_included,
            "plan_source": outcome.plan_source,
            "next_steps": next_steps,
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The run family every plan under test completes against; nothing
    /// here completes a task, so it carries no evidence capability.
    fn completion() -> crate::run::RunUseCases {
        crate::run::RunService::new(crate::run::engine::RunEngine::new("p".into())).use_cases()
    }

    // Priority *parsing* is unit-tested in infra::coerce. Here we only prove
    // the arg structs wire it up and deserialize infallibly.

    #[test]
    fn add_task_priority_named_or_numeric() {
        let named: AddTaskArgs =
            serde_json::from_str(r#"{"id":"x","title":"t","priority":"high"}"#).unwrap();
        assert_eq!(named.priority, 200);

        let numeric: AddTaskArgs =
            serde_json::from_str(r#"{"id":"x","title":"t","priority":42}"#).unwrap();
        assert_eq!(numeric.priority, 42);

        // Omitted priority falls back to the serde default (0).
        let absent: AddTaskArgs = serde_json::from_str(r#"{"id":"x","title":"t"}"#).unwrap();
        assert_eq!(absent.priority, 0);

        // An unrecognized level is NOT an error — it coerces to 0. A -32602
        // here would cancel sibling tool calls, so infallibility is the point.
        let urgent: AddTaskArgs =
            serde_json::from_str(r#"{"id":"x","title":"t","priority":"urgent"}"#).unwrap();
        assert_eq!(urgent.priority, 0);

        // id/title absent → "" (not -32602); handler soft-errors on empty id.
        let bare: AddTaskArgs = serde_json::from_str(r#"{}"#).unwrap();
        assert_eq!(bare.id, "");
        assert_eq!(bare.title, "");
    }

    #[test]
    fn content_input_schema_is_typed_and_still_described() {
        // The regression this guards: with `content` left untyped in the
        // input schema, schema-decoding clients (Claude Code) deliver the
        // object as a JSON-encoded STRING and every structured body is
        // rejected. The type declaration is what makes them send an object;
        // the doc-comment description must survive `schema_with` because it
        // is the only place the shape is steered from.
        for schema in [
            schemars::schema_for!(AddTaskArgs),
            schemars::schema_for!(UpdateTaskArgs),
        ] {
            let root = schema.to_value();
            let content = &root["properties"]["content"];
            assert_eq!(content["type"], serde_json::json!(["object", "null"]));
            let description = content["description"].as_str().unwrap_or_default();
            assert!(!description.is_empty(), "description lost: {root}");
        }

        // The shape itself is steered from exactly one place: the add tool.
        let add = schemars::schema_for!(AddTaskArgs).to_value();
        let steer = add["properties"]["content"]["description"]
            .as_str()
            .unwrap_or_default();
        assert!(steer.contains("version:1"), "shape line lost: {steer}");
    }

    #[test]
    fn reprioritize_priority_named_or_numeric() {
        let named: ReprioritizeArgs =
            serde_json::from_str(r#"{"id":"x","suggested_priority":"high","reason":"r"}"#).unwrap();
        assert_eq!(named.suggested_priority, 200);
    }

    // ── blocked-by-set-and-cleared ────────────────────────────────────────
    //
    // Driven through the TOOL seam — real wire JSON deserialized into
    // `UpdateTaskArgs`, then the handler — rather than through the engine
    // verbs, because everything the engine half does was already proven in
    // `chunk-blocked-by-vocabulary`. What is new here is the parameter
    // plumbing, and a parameter is exactly the thing an engine test cannot
    // reach: a field can be typed correctly and still be dropped, ignored,
    // or applied when it should not be.

    /// A service holding one task, ready to be patched.
    fn svc_with_task() -> PlanService {
        use crate::plan::domain::TaskStatus;
        use crate::plan::engine::PlanEngine;

        let mut engine = PlanEngine::new("p".into());
        engine
            .add_task(
                "c".into(),
                "A task".into(),
                TaskStatus::ToDo,
                10,
                "d".into(),
                vec![],
                vec![],
                false,
            )
            .expect("task added");
        PlanService::new(engine, completion())
    }

    /// Patch the task with raw wire JSON, exactly as a client would send it.
    async fn patch(svc: &PlanService, wire: &str) -> serde_json::Value {
        let args: UpdateTaskArgs = serde_json::from_str(wire).expect("wire deserializes");
        let mut answer = svc
            .plan_update_task(Parameters(args))
            .await
            .expect("handler returns")
            .structured_content
            .expect("structured content");
        if answer.get("ok") == Some(&serde_json::json!(false)) {
            return answer;
        }
        assert_eq!(answer["source"]["kind"], "memory");
        answer["value"].take()
    }

    #[tokio::test]
    async fn a_blocker_is_set_through_the_tool_seam() {
        let svc = svc_with_task();
        let out = patch(
            &svc,
            r#"{"id":"c","waiting_on":{"kind":"person","reason":"  needs a decision  ",
                "evidence":"decision:42"}}"#,
        )
        .await;

        let b = &out["waiting_on"];
        assert_eq!(b["kind"], "person");
        // Trimmed by the engine's validator, not by this seam — the seam is
        // asserted to PASS THROUGH rather than to clean up after the caller.
        assert_eq!(b["reason"], "needs a decision");
        assert_eq!(b["evidence"], "decision:42");
        assert!(
            !b["waiting_since"].as_str().unwrap_or_default().is_empty(),
            "a waiting reason must be stamped: {out}"
        );
    }

    #[tokio::test]
    async fn omitting_waiting_on_leaves_a_stored_blocker_alone() {
        // THE RULE THIS TOOL ALREADY FOLLOWS ON SIX OTHER FIELDS. Without it,
        // any agent editing a description would silently retract a waiting reason it
        // never mentioned — the failure that makes an omitted-means-unchanged
        // tool unsafe to use at all.
        let svc = svc_with_task();
        patch(
            &svc,
            r#"{"id":"c","waiting_on":{"kind":"outside","reason":"waiting on a vendor"}}"#,
        )
        .await;

        let out = patch(&svc, r#"{"id":"c","description":"an unrelated edit"}"#).await;
        assert_eq!(out["description"], "an unrelated edit");
        assert_eq!(
            out["waiting_on"]["reason"], "waiting on a vendor",
            "an unrelated patch retracted the waiting reason: {out}"
        );
    }

    #[tokio::test]
    async fn clearing_leaves_no_trace_on_disk() {
        // THE LOAD-BEARING PROPERTY. A clear that stored an emptied husk would
        // read as "blocked, reason unknown" to every later reader, which is
        // strictly worse than the prose this field replaced. The proof is
        // byte-level and derived: the whole record before and after, with only
        // the timestamps (which MUST move) lifted out.
        fn without_timestamps(mut v: serde_json::Value) -> serde_json::Value {
            let obj = v.as_object_mut().expect("task is an object");
            obj.remove("updated_at");
            v
        }

        let svc = svc_with_task();
        let pristine = patch(&svc, r#"{"id":"c"}"#).await;
        assert!(
            pristine.get("waiting_on").is_none(),
            "a task that was never blocked must carry no key: {pristine}"
        );

        patch(
            &svc,
            r#"{"id":"c","waiting_on":{"kind":"condition","reason":"no tenant yet"}}"#,
        )
        .await;
        let cleared = patch(&svc, r#"{"id":"c","clear_waiting_on":true}"#).await;

        assert!(
            cleared.get("waiting_on").is_none(),
            "clearing left a husk behind: {cleared}"
        );
        assert_eq!(
            without_timestamps(cleared),
            without_timestamps(pristine),
            "an no longer waiting task must be indistinguishable from one never blocked"
        );
    }

    #[tokio::test]
    async fn set_then_clear_then_set_again_through_the_tool_seam() {
        // The sequence a real task lives: blocked, no longer waiting when the premise
        // is re-tested, blocked again for a different reason. The second set is
        // the one that would fail if clearing had left the field in some
        // half-state the setter refused to overwrite.
        let svc = svc_with_task();
        for wire in [
            r#"{"id":"c","waiting_on":{"kind":"disproved","reason":"first"}}"#,
            r#"{"id":"c","clear_waiting_on":true}"#,
            r#"{"id":"c","waiting_on":{"kind":"person","reason":"second"}}"#,
        ] {
            patch(&svc, wire).await;
        }
        let out = patch(&svc, r#"{"id":"c"}"#).await;
        assert_eq!(out["waiting_on"]["kind"], "person");
        assert_eq!(out["waiting_on"]["reason"], "second");
    }

    #[tokio::test]
    async fn a_blocker_is_restated_without_clearing_first() {
        // Replacement, not refusal: the reason a task is stuck changes more
        // often than the fact that it is, and requiring a clear in between
        // would put a gesture between a human and the truth.
        let svc = svc_with_task();
        patch(
            &svc,
            r#"{"id":"c","waiting_on":{"kind":"condition","reason":"not yet"}}"#,
        )
        .await;
        let out = patch(
            &svc,
            r#"{"id":"c","waiting_on":{"kind":"disproved","reason":"never will be"}}"#,
        )
        .await;
        assert_eq!(out["waiting_on"]["kind"], "disproved");
        assert_eq!(out["waiting_on"]["reason"], "never will be");
    }

    #[tokio::test]
    async fn an_unrecognised_kind_names_every_legal_value() {
        // Derived from the vocabulary rather than spelled out, so a fifth kind
        // added later is covered here without anyone remembering to come back.
        use crate::plan::domain::WaitingReason;

        let svc = svc_with_task();
        let out = patch(
            &svc,
            r#"{"id":"c","waiting_on":{"kind":"blocked-by-vibes","reason":"r"}}"#,
        )
        .await;

        assert_eq!(out["ok"], false, "a bad kind must be refused: {out}");
        let msg = out["message"].as_str().unwrap_or_default();
        for kind in WaitingReason::ALL {
            assert!(
                msg.contains(kind.as_wire()),
                "the error must name '{}' among the legal values, got: {msg}",
                kind.as_wire()
            );
        }

        let after = patch(&svc, r#"{"id":"c"}"#).await;
        assert!(
            after.get("waiting_on").is_none(),
            "a refused kind must write nothing: {after}"
        );
    }

    #[tokio::test]
    async fn clearing_a_task_that_is_not_blocked_is_loud() {
        // Matching resolve_title_proposal: a caller who believes there is a
        // waiting reason to retract when there is none is working from a stale
        // picture, and a silent success would confirm it.
        let svc = svc_with_task();
        let out = patch(&svc, r#"{"id":"c","clear_waiting_on":true}"#).await;
        assert_eq!(out["ok"], false, "a no-op clear reported success: {out}");
        assert!(
            out["message"]
                .as_str()
                .unwrap_or_default()
                .contains("no waiting reason"),
            "the error must say what was missing: {out}"
        );
    }

    #[tokio::test]
    async fn setting_and_clearing_in_one_call_is_refused_before_anything_is_written() {
        let svc = svc_with_task();
        let out = patch(
            &svc,
            r#"{"id":"c","waiting_on":{"kind":"outside","reason":"r"},"clear_waiting_on":true}"#,
        )
        .await;
        assert_eq!(out["ok"], false, "a contradiction was accepted: {out}");

        let after = patch(&svc, r#"{"id":"c"}"#).await;
        assert!(
            after.get("waiting_on").is_none(),
            "the refused call still wrote a waiting reason: {after}"
        );
    }

    #[test]
    fn blocker_args_never_fail_deserialization() {
        // A hard -32602 out of deserialization cancels every sibling tool call
        // in a parallel batch, so malformed input must arrive at the handler
        // and come back as a readable soft error instead. Same rule as
        // `add_task_priority_named_or_numeric`.
        let empty: UpdateTaskArgs = serde_json::from_str(r#"{"id":"c","waiting_on":{}}"#).unwrap();
        let b = empty.waiting_on.expect("present");
        assert_eq!(b.kind, "");
        assert_eq!(b.reason, "");
        assert!(b.evidence.is_none());

        // The clear flag is lenient for the same reason a priority level is.
        let stringly: UpdateTaskArgs =
            serde_json::from_str(r#"{"id":"c","clear_waiting_on":"true"}"#).unwrap();
        assert!(stringly.clear_waiting_on);

        let absent: UpdateTaskArgs = serde_json::from_str(r#"{"id":"c"}"#).unwrap();
        assert!(absent.waiting_on.is_none());
        assert!(!absent.clear_waiting_on);
    }

    #[test]
    fn link_takes_ref_and_no_other_spelling() {
        let canonical: LinkArgs = serde_json::from_str(r#"{"id":"x","ref":"decision:5"}"#).unwrap();
        assert_eq!(canonical.reference, "decision:5");
        for retired in ["cross_ref", "crossref"] {
            let old: LinkArgs =
                serde_json::from_str(&format!(r#"{{"id":"x","{retired}":"decision:5"}}"#)).unwrap();
            assert!(old.reference.is_empty(), "{retired} is not read");
        }
    }

    // ── Focus at the wire seam ─────────────────────────────────────────

    /// Two grouped tasks in two features, plus one unassigned, so a leak in
    /// either direction is visible.
    fn svc_with_features() -> PlanService {
        use crate::plan::domain::TaskStatus;
        use crate::plan::engine::PlanEngine;

        let mut engine = PlanEngine::new("p".into());
        for (id, priority, feature) in [
            ("auth-1", 10u32, Some("Authentication")),
            ("auth-2", 30, Some("Authentication")),
            ("bill-1", 1, Some("Billing")),
            ("loose", 2, None),
        ] {
            engine
                .add_task(
                    id.into(),
                    format!("Task {id}"),
                    TaskStatus::ToDo,
                    priority,
                    String::new(),
                    vec![],
                    vec![],
                    false,
                )
                .expect("task added");
            if let Some(g) = feature {
                engine
                    .set_feature(id, Some(g.to_string()))
                    .expect("these names are real places, not id prefixes");
            }
        }
        PlanService::new(engine, completion())
    }

    async fn focus_get(svc: &PlanService, wire: &str) -> serde_json::Value {
        let args: FocusGetArgs = serde_json::from_str(wire).expect("wire deserializes");
        svc.plan_focus_get(Parameters(args))
            .await
            .expect("handler returns")
            .structured_content
            .expect("structured content")
    }

    async fn focus_set(svc: &PlanService, wire: &str) -> serde_json::Value {
        let args: FocusSetArgs = serde_json::from_str(wire).expect("wire deserializes");
        let mut answer = svc
            .plan_focus_set(Parameters(args))
            .await
            .expect("handler returns")
            .structured_content
            .expect("structured content");
        if answer.get("ok") == Some(&serde_json::json!(false)) {
            return answer;
        }
        assert_eq!(answer["source"]["kind"], "memory");
        answer["value"].take()
    }

    /// A task created over the wire WITH a feature is focusable in one call.
    ///
    /// The reported symptom was six tasks added for a feature, all landing
    /// unassigned, all invisible to the focused frontier — and six follow-up
    /// `plan_set_feature` calls needed to repair it. This is that whole flow
    /// through the tool seam, in one call.
    #[tokio::test]
    async fn a_task_added_with_a_feature_is_focusable_without_a_second_call() {
        use crate::plan::engine::PlanEngine;
        let svc = PlanService::new(PlanEngine::new("p".into()), completion());

        let args: AddTaskArgs = serde_json::from_str(
            r#"{"id":"auth-1","title":"Rotate sessions","priority":10,"feature":"Authentication"}"#,
        )
        .expect("wire deserializes");
        let created = svc
            .plan_add_task(Parameters(args))
            .await
            .unwrap()
            .structured_content
            .unwrap();
        assert_eq!(created["value"]["feature"], "Authentication");
        assert_eq!(created["source"]["kind"], "memory");

        // Focus and read the frontier: the task is there, with no set_feature.
        let focused = focus_set(
            &svc,
            r#"{"team":"/w/a","feature":"Authentication","mode":"build"}"#,
        )
        .await;
        assert_eq!(focused["frontier"]["activity"]["next_ready"], "auth-1");
        assert_eq!(focused["frontier"]["activity"]["readiness"]["ready"], 1);
    }

    /// Omitting `feature` is unchanged behaviour — every existing caller keeps
    /// working and its tasks stay unassigned.
    #[tokio::test]
    async fn omitting_the_feature_still_creates_an_unassigned_task() {
        use crate::plan::engine::PlanEngine;
        let svc = PlanService::new(PlanEngine::new("p".into()), completion());
        let args: AddTaskArgs =
            serde_json::from_str(r#"{"id":"loose","title":"t","priority":1}"#).unwrap();
        let created = svc
            .plan_add_task(Parameters(args))
            .await
            .unwrap()
            .structured_content
            .unwrap();
        assert!(
            created["value"]
                .get("feature")
                .is_none_or(serde_json::Value::is_null),
            "an omitted feature must leave the task unassigned, not invent one"
        );
    }

    /// An unusable feature refuses the whole call over the wire — it does not
    /// quietly create the task without one.
    #[tokio::test]
    async fn an_unusable_feature_refuses_the_add_over_the_wire() {
        use crate::plan::engine::PlanEngine;
        let svc = PlanService::new(PlanEngine::new("p".into()), completion());
        let args: AddTaskArgs = serde_json::from_str(
            r#"{"id":"billing-1","title":"t","priority":1,"feature":"Billing"}"#,
        )
        .unwrap();
        let res = svc
            .plan_add_task(Parameters(args))
            .await
            .unwrap()
            .structured_content
            .unwrap();
        assert_eq!(res["ok"], false);
        assert_eq!(res["error_kind"], "invalid_state");

        // Nothing was created — not even an unassigned fallback.
        let status = svc
            .plan_status(Parameters(NoArgs {}))
            .await
            .unwrap()
            .structured_content
            .unwrap();
        assert_eq!(status["counts"]["total"], 0);
    }

    /// The happy path, end to end over the wire: set, then read back.
    #[tokio::test]
    async fn setting_a_focus_returns_the_team_the_mode_and_its_frontier() {
        let svc = svc_with_features();
        let set = focus_set(
            &svc,
            r#"{"team":"/w/a","feature":"Authentication","mode":"build"}"#,
        )
        .await;
        // Success carries no `ok` envelope — only a soft failure sets one, so
        // the absence of `ok:false` IS the success signal here.
        assert!(set.get("ok").is_none(), "success must not report an error");
        assert_eq!(set["team"], "/w/a");
        assert_eq!(set["focus"]["feature"], "Authentication");
        assert_eq!(set["focus"]["mode"], "build");
        // The frontier is the FOCUSED feature's, not the plan's: bill-1
        // has the best priority overall and must not appear.
        assert_eq!(set["frontier"]["activity"]["next_ready"], "auth-1");
        assert_eq!(set["frontier"]["activity"]["readiness"]["ready"], 2);

        let got = focus_get(&svc, r#"{"team":"/w/a"}"#).await;
        assert_eq!(got["focus"]["feature"], "Authentication");
        assert_eq!(got["focus_state"]["state"], "current");
        assert_eq!(got["frontier"]["state"], "current");
        assert_eq!(
            got["frontier"]["frontier"]["activity"]["next_ready"],
            "auth-1"
        );
    }

    /// A read is a read. `plan_focus_get` must not create a focus as a side
    /// effect of being asked about one.
    #[tokio::test]
    async fn reading_a_focus_never_creates_one() {
        let svc = svc_with_features();
        let got = focus_get(&svc, r#"{"team":"/w/never-focused"}"#).await;
        assert_eq!(got["focus"], serde_json::Value::Null);
        assert_eq!(got["focus_state"]["state"], "unset");
        assert_eq!(got["frontier"]["state"], "unset");
        assert!(got["features"]["current"].get("Billing").is_some());
        assert_eq!(got["features"]["unassigned"]["counts"]["total"], 1);
        // Still nothing stored, so a second read agrees with the first.
        assert_eq!(
            focus_get(&svc, r#"{"team":"/w/never-focused"}"#).await["focus"],
            serde_json::Value::Null
        );
    }

    /// Peeking at another feature reports it WITHOUT switching to it.
    #[tokio::test]
    async fn peeking_at_a_feature_reports_it_without_switching() {
        let svc = svc_with_features();
        focus_set(
            &svc,
            r#"{"team":"/w/a","feature":"Authentication","mode":"shape"}"#,
        )
        .await;
        let peek = focus_get(&svc, r#"{"team":"/w/a","feature":"Billing"}"#).await;
        assert_eq!(peek["frontier"]["frontier"]["feature"], "Billing");
        // The focus itself did not move.
        assert_eq!(peek["focus"]["feature"], "Authentication");
        assert_eq!(peek["focus"]["mode"], "shape");
        assert_eq!(
            peek["focus_state"],
            serde_json::json!({
                "state": "current", "feature": "Authentication",
            })
        );
    }

    /// An unknown feature is a soft error carrying the candidates, and it
    /// leaves an existing focus exactly where it was.
    #[tokio::test]
    async fn an_unknown_or_ambiguous_feature_is_refused_over_the_wire_without_mutating() {
        let svc = svc_with_features();
        focus_set(
            &svc,
            r#"{"team":"/w/a","feature":"Authentication","mode":"build"}"#,
        )
        .await;

        let bad = focus_set(
            &svc,
            r#"{"team":"/w/a","feature":"Payments","mode":"build"}"#,
        )
        .await;
        assert_eq!(bad["ok"], false);
        assert_eq!(bad["error_kind"], "invalid_args");
        let msg = bad["message"].as_str().unwrap();
        assert!(msg.contains("Authentication") && msg.contains("Billing"));

        // Untouched.
        let got = focus_get(&svc, r#"{"team":"/w/a"}"#).await;
        assert_eq!(got["focus"]["feature"], "Authentication");
        assert_eq!(got["focus"]["mode"], "build");
    }

    /// A blank team is refused on BOTH verbs, including the clear path — the
    /// clear path is the one where a default would be most tempting and most
    /// destructive, since it would release somebody else's focus.
    #[tokio::test]
    async fn a_blank_team_is_refused_on_set_and_on_clear() {
        let svc = svc_with_features();
        focus_set(
            &svc,
            r#"{"team":"/w/a","feature":"Authentication","mode":"build"}"#,
        )
        .await;

        let set = focus_set(&svc, r#"{"team":"","feature":"Billing","mode":"build"}"#).await;
        assert_eq!(set["ok"], false);
        let clear = focus_set(&svc, r#"{"team":"  ","clear":true}"#).await;
        assert_eq!(clear["ok"], false);

        // The real team still holds its focus.
        assert_eq!(
            focus_get(&svc, r#"{"team":"/w/a"}"#).await["focus"]["feature"],
            "Authentication"
        );
    }

    /// The retired caller key `lane` is not a spelling of `team`: a set that
    /// uses it names no team and is refused without writing anything, and a
    /// read that uses it names no team that holds a focus.
    #[tokio::test]
    async fn the_retired_lane_argument_is_refused_not_translated() {
        let svc = svc_with_features();
        let set = focus_set(
            &svc,
            r#"{"lane":"/w/a","feature":"Authentication","mode":"build"}"#,
        )
        .await;
        assert_eq!(set["ok"], false, "{set}");
        focus_set(
            &svc,
            r#"{"team":"/w/a","feature":"Billing","mode":"build"}"#,
        )
        .await;
        assert!(focus_get(&svc, r#"{"lane":"/w/a"}"#).await["focus"].is_null());
        assert_eq!(
            focus_get(&svc, r#"{"team":"/w/a"}"#).await["focus"]["feature"],
            "Billing",
            "only the team key reaches the saved focus"
        );
    }

    #[tokio::test]
    async fn a_set_under_the_retired_key_writes_nothing() {
        let svc = svc_with_features();
        focus_set(
            &svc,
            r#"{"lane":"/w/a","feature":"Authentication","mode":"build"}"#,
        )
        .await;
        assert!(
            focus_get(&svc, r#"{"team":"/w/a"}"#).await["focus"].is_null(),
            "nothing was focused under the old key"
        );
    }

    /// An unknown mode is refused with the vocabulary, and no synonym is taken.
    #[tokio::test]
    async fn an_unknown_mode_is_refused_with_the_vocabulary() {
        let svc = svc_with_features();
        let bad = focus_set(
            &svc,
            r#"{"team":"/w/a","feature":"Authentication","mode":"implement"}"#,
        )
        .await;
        assert_eq!(bad["ok"], false);
        let msg = bad["message"].as_str().unwrap();
        assert!(msg.contains("shape") && msg.contains("build") && msg.contains("listen"));
        assert_eq!(
            focus_get(&svc, r#"{"team":"/w/a"}"#).await["focus"],
            serde_json::Value::Null,
            "a refused mode must not have created a focus"
        );
    }

    /// Clearing releases only the asking team.
    #[tokio::test]
    async fn clearing_releases_only_the_asking_team() {
        let svc = svc_with_features();
        focus_set(
            &svc,
            r#"{"team":"/w/a","feature":"Authentication","mode":"build"}"#,
        )
        .await;
        focus_set(
            &svc,
            r#"{"team":"/w/b","feature":"Billing","mode":"listen"}"#,
        )
        .await;

        let cleared = focus_set(&svc, r#"{"team":"/w/a","clear":true}"#).await;
        assert_eq!(cleared["released"], true);
        assert_eq!(
            focus_get(&svc, r#"{"team":"/w/a"}"#).await["focus"],
            serde_json::Value::Null
        );
        assert_eq!(
            focus_get(&svc, r#"{"team":"/w/b"}"#).await["focus"]["feature"],
            "Billing"
        );
    }

    /// Caller focus does not change the unscoped next/status query results.
    #[tokio::test]
    async fn plan_next_and_status_keep_their_no_input_behaviour() {
        let svc = svc_with_features();
        let before_next = svc
            .plan_next(Parameters(NoArgs {}))
            .await
            .unwrap()
            .structured_content
            .unwrap();
        focus_set(
            &svc,
            r#"{"team":"/w/a","feature":"Authentication","mode":"build"}"#,
        )
        .await;
        let after_next = svc
            .plan_next(Parameters(NoArgs {}))
            .await
            .unwrap()
            .structured_content
            .unwrap();

        // Focusing does not narrow the GLOBAL next — the narrowing lives in the
        // focus frontier, so an existing caller sees no behaviour change.
        assert_eq!(before_next["id"], "bill-1");
        assert_eq!(after_next["id"], "bill-1");

        let status = svc
            .plan_status(Parameters(NoArgs {}))
            .await
            .unwrap()
            .structured_content
            .unwrap();
        assert_eq!(status["counts"]["total"], 4);
    }

    /// The compact summary carries the feature, so a reader building a
    /// focused frontier from `plan_status` can tell the rows apart.
    #[tokio::test]
    async fn compact_task_summaries_carry_the_feature() {
        let svc = svc_with_features();
        let status = svc
            .plan_status(Parameters(NoArgs {}))
            .await
            .unwrap()
            .structured_content
            .unwrap();
        let rows = status["tasks"].as_array().unwrap();
        let row = |id: &str| rows.iter().find(|r| r["id"] == id).expect("row present");
        assert_eq!(row("auth-1")["feature"], "Authentication");
        assert_eq!(row("bill-1")["feature"], "Billing");
        // Unassigned rows pay nothing rather than carrying a null.
        assert!(row("loose").get("feature").is_none());
    }

    #[tokio::test]
    async fn plan_status_folds_in_ready_request_count() {
        use crate::plan::engine::PlanEngine;
        use crate::request::domain::RequestKind;
        use crate::request::engine::RequestEngine;
        use std::sync::{Arc, Mutex};

        // A request engine with one researched, above-threshold request.
        let mut se = RequestEngine::new("p".into());
        let id = se
            .capture(RequestKind::Suggestion, "x".into(), "do it".into())
            .as_ref()
            .unwrap()
            .value
            .id
            .clone();
        se.research(&id, "looked".into(), 0.9, vec![], None)
            .unwrap();

        let svc = PlanService::new(PlanEngine::new("p".into()), completion())
            .with_request(Arc::new(Mutex::new(se)));
        let res = svc.plan_status(Parameters(NoArgs {})).await.unwrap();
        let sc = res.structured_content.expect("structured content");
        assert_eq!(sc["ready_requests"], 1);
    }

    // ── tracker_setup (tracker-mcp-setup) ─────────────────────────────────
    //
    // These adapter properties protect the JSON-RPC session. Concurrent native
    // access during the remote probe is exercised at the application boundary.

    /// The production half of this file, so an assertion string naming a call
    /// cannot satisfy the assertion that looks for it.
    fn handler_source() -> &'static str {
        let whole = include_str!("handlers.rs");
        whole.split_once("\nmod tests").map_or(whole, |(b, _)| b)
    }

    /// TRAP 1. `confirm` reads STDIN, and in a stdio server stdin carries the
    /// JSON-RPC frames — a prompt reached from a handler eats protocol traffic
    /// and desynchronizes the session. It must be unreachable from here.
    ///
    /// HONEST SCOPE. The stdin prompt `confirm` was deleted with the standalone CLI; before that it was
    /// PRIVATE to the `cli` module, so this module cannot call it at all — the
    /// deliberately-broken build that proves this test bites had to make it
    /// `pub` first. Rust's privacy is the real guard; this test is what
    /// survives somebody widening that visibility for an unrelated reason.
    ///
    /// Worth recording what deliberately breaking it actually showed, because
    /// it is the failure mode itself: with `confirm` reachable, the EXECUTION
    /// test below did not fail — it HUNG, blocking forever on a stdin read.
    /// That is
    /// precisely what an MCP session would do. This source scan fails in 0.00s
    /// instead, which is the whole reason to keep a structural check alongside
    /// a behavioural one.
    #[test]
    fn no_tool_handler_can_prompt_on_stdin() {
        let src = handler_source();
        assert!(
            !src.contains("confirm("),
            "a tool handler reaches `confirm`, which reads stdin. In a stdio MCP \
             server stdin IS the transport, so this hangs or desyncs the session \
             rather than asking anybody anything. Consent must arrive as a \
             parameter (`create_missing`)"
        );
        // And the flag that skips the CLI's prompt must not be settable here —
        // there is no prompt to skip, so a true would only be confusing.
        assert!(
            src.contains("yes: false"),
            "the setup request built here must hard-code `yes: false`; the MCP \
             path has no prompt, and `create_missing` is the whole decision"
        );
    }

    /// TRAP 2. `run_stdio` hands STDOUT to the transport, so printing from a
    /// handler injects text into the protocol stream.
    #[test]
    fn no_tool_handler_writes_to_stdout() {
        let src = handler_source();
        for printer in ["println!", "print!"] {
            assert!(
                !src.contains(printer),
                "a tool handler calls `{printer}`. stdout is the JSON-RPC \
                 transport — this corrupts the stream. Return the value in \
                 structuredContent, or use `eprintln!` as the engine does"
            );
        }
    }

    /// The default that matters: an agent that omits `create_missing` cannot
    /// provision anything, and a missing destination writes NOTHING.
    #[tokio::test]
    async fn a_missing_destination_writes_nothing_when_create_missing_is_omitted() {
        use crate::plan::engine::PlanEngine;
        let svc = PlanService::new(PlanEngine::new("p".into()), completion());

        // Deserialized from the wire WITHOUT create_missing, which is how an
        // agent that has not been told to provision will call it.
        let args: TrackerSetupArgs =
            serde_json::from_value(serde_json::json!({ "into": "acme/widgets" }))
                .expect("args parse");
        assert!(
            !args.create_missing,
            "create_missing MUST default to false — an agent is further from a \
             human than a CLI user, and this writes to somebody else's system"
        );
        assert_eq!(
            args.provider, "github",
            "provider defaults like its siblings"
        );
        assert!(
            args.push_secs.is_none(),
            "an omitted push interval is nothing, not a default cadence"
        );

        // Driving the handler needs no network for the assertion that matters:
        // whatever happens, mirroring must not be silently switched on for a
        // destination nobody confirmed.
        let res = svc.tracker_setup(Parameters(args)).await.expect("no panic");
        let sc = res.structured_content.expect("structured content");
        assert_ne!(
            sc.get("target_created")
                .and_then(serde_json::Value::as_bool),
            Some(true),
            "nothing may be provisioned without an explicit create_missing"
        );
    }

    /// Magistr runs no unattended tracker push. A caller that still sends the
    /// retired interval — an agent following an older description — is told
    /// so by name and nothing is written, rather than the setting being
    /// accepted and silently doing nothing.
    #[tokio::test]
    async fn a_supplied_push_interval_is_refused_by_name_and_writes_nothing() {
        use crate::plan::engine::PlanEngine;
        let svc = PlanService::new(PlanEngine::new("p".into()), completion());
        let args: TrackerSetupArgs = serde_json::from_value(
            serde_json::json!({ "into": "acme/widgets", "push_secs": 300, "dry_run": true }),
        )
        .expect("the retired field still deserializes, so it can be refused");

        let res = svc.tracker_setup(Parameters(args)).await.expect("no panic");
        assert_eq!(res.is_error, Some(true));
        let sc = res.structured_content.expect("structured content");
        assert_eq!(sc["ok"], serde_json::json!(false));
        assert_eq!(sc["error_kind"], serde_json::json!("unsupported_setting"));
        let message = sc["message"].as_str().unwrap_or_default().to_string();
        assert!(
            message.contains("push_secs") && message.contains("no unattended"),
            "the refusal must name the setting and the missing capability: {message}"
        );
        assert_eq!(
            sc["effect"],
            serde_json::json!("none"),
            "a refused setting is refused before any write"
        );
    }

    /// The served contract advertises only what the host does: no push
    /// interval on the way in, no auto-push report on the way out, and a
    /// description that promises no unattended pushing.
    #[test]
    fn the_tracker_setup_contract_promises_no_unattended_push() {
        use crate::plan::engine::PlanEngine;
        let svc = PlanService::new(PlanEngine::new("p".into()), completion());
        let tools = svc.list_tools_view();
        let tool = tools
            .iter()
            .find(|t| t.name == "tracker_setup")
            .expect("tracker_setup is served");

        let input = serde_json::to_string(&tool.input_schema).expect("input schema");
        assert!(
            !input.contains("push_secs") && !input.contains("push"),
            "the input schema still advertises a push setting: {input}"
        );
        let output = tool
            .output_schema
            .as_ref()
            .map(|s| serde_json::to_string(s).expect("output schema"))
            .unwrap_or_default();
        assert!(
            !output.contains("auto_push"),
            "the output schema still carries auto-push fields: {output}"
        );
        let description = tool.description.as_deref().unwrap_or_default();
        for promise in ["unattended", "push_secs", "auto_push", "wire up"] {
            assert!(
                !description.contains(promise),
                "the description still says `{promise}`: {description}"
            );
        }
        assert!(
            description.contains("Nothing is sent"),
            "the description must say that nothing is sent: {description}"
        );
    }

    #[tokio::test]
    async fn tracker_status_reports_when_nothing_is_mirrored() {
        use crate::plan::engine::PlanEngine;
        let svc = PlanService::new(PlanEngine::new("p".into()), completion());
        let res = svc.tracker_status(Parameters(NoArgs {})).await.unwrap();
        let sc = res.structured_content.expect("structured content");
        assert!(
            sc.get("enabled").is_some() && sc.get("included").is_some(),
            "status must always answer both questions, even when off: {sc}"
        );
    }

    #[tokio::test]
    async fn plan_status_omits_count_when_request_unwired() {
        use crate::plan::engine::PlanEngine;
        let svc = PlanService::new(PlanEngine::new("p".into()), completion());
        let res = svc.plan_status(Parameters(NoArgs {})).await.unwrap();
        let sc = res.structured_content.expect("structured content");
        assert!(sc.get("ready_requests").is_none());
    }
}
