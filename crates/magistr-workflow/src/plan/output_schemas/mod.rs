//! `outputSchema` definitions for the `plan_*` tools, mirroring the
//! `run` family's approach: each JSON-returning tool advertises a schema so
//! 2026 clients can parse `structuredContent` instead of the text fallback.

use magistr_workflow_contracts::Observed;
use std::sync::Arc;

use rmcp::model::JsonObject;
use schemars::{JsonSchema, schema_for};
use serde::Serialize;
use serde_json::Value;

// WHY THE TWO FOCUS TOOLS ADVERTISE NO `outputSchema`, MEASURED TWICE.
//
// They return `structuredContent` like every sibling; what is missing is only
// the advertisement, and it is missing for a priced reason rather than an
// oversight — the same class of refusal recorded for `waiting_on` below.
//
// Both alternatives were built and measured against the 154,000 B ceiling the
// focus verbs land 370 B under:
//
//   * fully described (`focus` a five-field record, `frontier` counts + ready
//     + blocked + next):                    +2,406 B  ->  156,036  (over)
//   * minimal envelope, both rich fields as
//     bare `Value` (schemars emits `true`):  +1,096 B  ->  154,726  (over)
//
// MCP cannot `$ref` across tools, so even the minimal envelope is paid twice.
// 1,096 B does not fit in 370 B, and the ceiling had just been moved once by an
// explicit human decision for this very task — moving it a second time in the
// same task to buy back what the first move was granted for is exactly the
// behaviour the rule on `tools_list_payload_stays_within_budget` forbids.
//
// So the shapes are documented in the tool descriptions, which is where a model
// reads them anyway (`outputSchema` is dropped by clients bridging to the
// Messages API before the model sees it — see `model_facing_bytes`). This entry
// becomes payable the moment the duplicated `request_*` schemas are reclaimed.

#[derive(Serialize, JsonSchema)]
pub struct ReprioritizeProposalOutput {
    pub suggested_priority: u32,
    pub reason: String,
    pub proposed_at: String,
}

/// A full task record, as returned by the mutating tools and `plan_next`.
#[derive(Serialize, JsonSchema)]
pub struct TaskOutput {
    pub id: String,
    pub title: String,
    /// One of: ideas, pending, in_progress, blocked, done, retired.
    pub status: String,
    pub priority: u32,
    pub description: String,
    pub acceptance: Vec<String>,
    pub deps: Vec<String>,
    pub refs: Vec<String>,
    pub shared: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reprioritize: Option<ReprioritizeProposalOutput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retired_reason: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

// WHY `TaskOutput` DOES NOT DESCRIBE `waiting_on`, THOUGH THE RESPONSE CARRIES IT.
//
// The handlers return `serde_json::to_value(task)` — the real `Task` — so a
// blocked task's response DOES include `waiting_on`. What is missing is only
// its ADVERTISEMENT here, and that omission was measured rather than assumed:
// adding a four-field `WaitingOnOutput` to this struct moved `tools_list`
// from 147,267 B to 156,337 B — 9,070 B for four fields, because `TaskOutput`
// is inlined into roughly ten plan tool schemas and MCP cannot `$ref` a
// schema across tools. That is 6,337 B through a 150,000 B ceiling whose
// standing rule is "re-decide the surface, don't raise the line".
//
// So this is a known, priced incompleteness, not an oversight — and it is the
// same one `feature`, `name`, `content` and `notes` already have. The honest fix
// is a shared task schema rather than ten copies of one, which is a surface
// decision of its own; `chunk-output-schema-is-a-partial-projection` records it.

#[derive(Serialize, JsonSchema)]
pub struct PlanCounts {
    pub ideas: usize,
    pub pending: usize,
    pub in_progress: usize,
    pub blocked: usize,
    pub done: usize,
    pub retired: usize,
    pub total: usize,
    /// Waiting reason-carrying tasks on the ACTIVE board, by kind. Cross-cutting, not
    /// a seventh bucket: a blocked task keeps its status and is counted above.
    pub waiting_on: BlockerCounts,
}

// WHY THE TALLY IS BROKEN DOWN AND NOT LUMPED. The kinds mean different things
// to a reader deciding what to do next: `disproved` is a result and wants
// re-scoping, `person` is a queue and wants a person, and one total
// cannot tell them apart. Every kind is reported even at zero, because absence
// and zero are different answers — the engine seeds the map from
// `WaitingReason::ALL` rather than from what happens to be on the board.
//
// Doc-comment budget: this rationale is a `//`, not a `///`, because every
// `///` here is serialized into the `plan_status` output schema and shipped
// in the tools/list payload against a 150,000 B ceiling.
/// The by-kind waiting reason tally. Every kind is reported, at 0 when nothing carries it.
#[derive(Serialize, JsonSchema)]
pub struct BlockerCounts {
    pub total: usize,
    pub disproved: usize,
    pub condition: usize,
    pub person: usize,
    pub external: usize,
}

#[derive(Serialize, JsonSchema)]
pub struct TaskSummary {
    pub id: String,
    pub title: String,
    /// The short node label. Present in the summary alongside `title` because a
    /// caller rendering a list of tasks is exactly the caller that needs a
    /// label rather than 519 sentences.
    pub name: String,
    pub status: String,
    pub priority: u32,
    /// The band `priority` falls in: critical | high | medium | low | later.
    /// Names the number so a reader doesn't have to know the scale.
    pub band: String,
    pub deps: Vec<String>,
    pub has_reprioritize_proposal: bool,
    /// Whether a tracker sweep has suggested a different status for this task.
    /// A proposal is NEVER a transition — `status` above is still the truth.
    pub has_status_proposal: bool,
    /// Whether a contested tracker title has been conceded and awaits a human
    /// decision. While open, projections keep sending the tracker's title.
    pub has_title_proposal: bool,
    // Replaces the `has_blocker: bool` this field grew out of. The boolean was
    // exactly `waiting_on.is_some()`, so the two could never legally disagree
    // and one of them was a fifth wheel; the kind subsumes it and answers the
    // question it left open. Rationale lives in `//` for the budget reason
    // noted on `BlockerCounts` above.
    /// Why this task cannot be worked: `disproved` | `condition` |
    /// `person` | `external`, absent when nothing blocks it. Such a
    /// task is never what `plan_next` answers but keeps its priority and
    /// place — skipped is not hidden — and since `title` is truncated and a
    /// waiting reason is usually stated in a title's tail, this token is what survives.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub waiting_on: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct PlanStatusOutput {
    pub project_id: String,
    pub counts: PlanCounts,
    /// Id of the next ready task (most urgent — smallest `priority` number —
    /// no longer waiting, deps satisfied), if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
    /// Total count of active (ideas/pending/in_progress/blocked) tasks.
    pub active_total: usize,
    /// Active tasks beyond the cap that are not included in `tasks`.
    pub omitted_active: usize,
    /// Active tasks only, priority-sorted, titles truncated, capped in length.
    /// Done/retired tasks are summarized in `counts` + `recent_done`, not here,
    /// so the snapshot stays within the MCP output limit on a large plan.
    pub tasks: Vec<TaskSummary>,
    /// A short tail of the most-recently-finished (done/retired) tasks.
    pub recent_done: Vec<TaskSummary>,
    /// Human-readable note when anything was elided (empty otherwise).
    pub note: String,
}

#[derive(Serialize, JsonSchema)]
#[serde(untagged)]
pub enum PlanExportOutput {
    File {
        format: String,
        path: String,
        bytes: usize,
        written: bool,
    },
    Inline {
        format: String,
        plan: String,
    },
    Error {
        ok: bool,
        error_kind: String,
        message: String,
    },
}

// WHY `records` IS A BARE `Value` AND NOT A `Vec<TaskOutput>`.
//
// Two reasons, and the first is correctness rather than thrift. `fields`
// projects each record, so the shape genuinely VARIES per call — a schema
// enumerating twenty-two task keys would describe a response this tool only
// returns when no projection was asked for, and a schema that is wrong most of
// the time is worse than one that declines to guess.
//
// The second is the price recorded above: `TaskOutput` inlines into ~10
// plan tool schemas and MCP cannot `$ref` across tools, so reaching for it
// here would have added an eleventh copy. schemars emits `true` for a `Value`,
// which costs almost nothing, and the fields that DO NOT vary are enumerated
// properly below — those are the ones a client can actually validate against.
/// `plan_get` — the bounded, projectable fetch.
#[derive(Serialize, JsonSchema)]
pub struct PlanGetOutput {
    /// The stored records, in the order `ids` named them. Shape follows
    /// `fields`; `id` is present whatever was projected.
    pub records: Value,
    pub returned: usize,
    /// Ids that matched no task. Reported rather than omitted, so a caller can
    /// tell "no such task" from "that task has no summary".
    pub unknown: Vec<String>,
    /// The projection that was applied; empty means the whole record.
    pub fields: Vec<String>,
}

/// `plan_start_task`: the task plus the reference the agent wires into
/// run/decision.
#[derive(Serialize, JsonSchema)]
pub struct StartTaskOutput {
    pub task: TaskOutput,
    /// `task:<id>` — pass to run_open + a decision run_ref.
    #[serde(rename = "ref")]
    pub reference: String,
    pub hint: String,
}

#[derive(Serialize, JsonSchema)]
pub struct RefreshOutput {
    pub recorded: bool,
    pub summary: String,
    pub notes: Vec<u32>,
    pub total_refreshes: usize,
}

#[derive(Serialize, JsonSchema)]
pub struct StructuredError {
    pub error_kind: String,
    pub message: String,
}

/// `plan_propose_features` — which unassigned tasks belong together, for a
/// person to name.
#[derive(Serialize, JsonSchema)]
pub struct ProposeFeaturesOutput {
    /// One entry per cluster: the shared id prefix, the unassigned task ids in
    /// it, and why that prefix cannot be the name of the region holding them.
    pub clusters: Value,
    /// The threshold that was applied: a cluster needed at least this many
    /// unassigned tasks to be worth naming.
    pub min_shared: usize,
    /// Current and historical names for avoiding collisions, not current choices.
    pub authored_names: Vec<String>,
    /// Said in the response rather than left to `doctor`: this call places
    /// nothing, because no name derived from an id prefix can pass the region
    /// check.
    pub placed: usize,
}

/// `tracker_status` — read-only mirroring state.
#[derive(Serialize, JsonSchema)]
pub struct TrackerStatusOutput {
    pub project_id: String,
    /// Whether mirroring would actually push. False is the default: tracker
    /// projection is opt-in, so `provider`/`target` can be set while this is
    /// still false.
    pub enabled: bool,
    pub provider: Option<String>,
    pub target: Option<String>,
    /// How many tasks are opted in to this provider.
    pub included: usize,
    /// THE DRIFT: how many ACTIVE tasks are not opted in, and are therefore
    /// invisible to the tracker. Reported alongside `included` because
    /// `included` only ever grows and so can never reveal a scope that stopped
    /// growing — which is exactly how this went unnoticed at 46 of 66.
    pub not_included: usize,
    /// Whether newly-created tasks are born in scope. True only for a project
    /// that EXPLICITLY ran `tracker on` / `tracker setup`.
    pub inherits_new_tasks: bool,
}

/// `tracker_setup` — what the setup run actually did.
///
/// The three `target_*` booleans are mutually exclusive and one is always
/// true: the destination was found, created, or could not be verified. They
/// are separate fields rather than an enum because that is the JSON the
/// handler builds, and a schema that improved on it would be a lie.
#[derive(Serialize, JsonSchema)]
pub struct TrackerSetupOutput {
    pub project_id: String,
    pub provider: String,
    pub target: String,
    /// True when nothing was written anywhere.
    pub dry_run: bool,
    pub target_existed: bool,
    /// True only when `create_missing` was set AND the destination was absent.
    pub target_created: bool,
    /// The destination could not be checked; nothing was created on a guess.
    pub target_unverified: bool,
    pub mirroring_enabled: bool,
    /// Task ids newly opted in by this run.
    pub included: Vec<String>,
    /// Task ids that were already opted in.
    pub already_included: usize,
    pub plan_source: Option<magistr_workflow_contracts::ReadSource>,
    /// Follow-up a human must perform — notably that nothing has been sent,
    /// because this host runs no background push.
    pub next_steps: Vec<String>,
}

pub fn output_schema_for(tool_name: &str) -> Option<Arc<JsonObject>> {
    let value: Value = match tool_name {
        "plan_add_task" | "plan_set_status" | "plan_update_task" | "plan_retire_task"
        | "plan_reprioritize" | "plan_complete_task" | "plan_link" => {
            schema_for!(Observed<TaskOutput>).to_value()
        }
        "plan_next" => schema_for!(TaskOutput).to_value(),
        "plan_status" => schema_for!(PlanStatusOutput).to_value(),
        "plan_export" => schema_for!(PlanExportOutput).to_value(),
        "plan_get" => schema_for!(PlanGetOutput).to_value(),
        "plan_start_task" => schema_for!(Observed<StartTaskOutput>).to_value(),
        "plan_record_refresh" => schema_for!(Observed<RefreshOutput>).to_value(),
        // plan_set_feature returns the patched task, exactly as the other
        // task-mutating verbs above do.
        "plan_set_feature" => schema_for!(Observed<TaskOutput>).to_value(),
        "plan_propose_features" => schema_for!(ProposeFeaturesOutput).to_value(),
        // tracker_* rides this family (see UnifiedService::route_of), so its
        // schemas live here rather than in a fifth module.
        "tracker_status" => schema_for!(TrackerStatusOutput).to_value(),
        "tracker_setup" => schema_for!(TrackerSetupOutput).to_value(),
        _ => return None,
    };
    match value {
        Value::Object(mut map) => {
            if tool_name == "plan_export" {
                map.insert("type".into(), Value::String("object".into()));
            }
            Some(Arc::new(map))
        }
        _ => None,
    }
}
