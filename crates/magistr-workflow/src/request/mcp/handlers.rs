//! The `request_*` tool handlers — rmcp `#[tool]` methods over [`RequestService`],
//! mirroring `plan::mcp::handlers`. Args deserialize forgivingly (absent
//! fields become defaults, never a -32602 that would cancel sibling calls);
//! logical failures return a soft `{ok:false, …}` envelope.

use rmcp::{
    ErrorData, handler::server::wrapper::Parameters, model::CallToolResult, tool, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;

use super::service::RequestService;
use crate::request::domain::RequestKind;

impl RequestService {
    pub(crate) fn make_tool_router() -> rmcp::handler::server::router::tool::ToolRouter<Self> {
        Self::tool_router()
    }
}

/// Parse a wire kind string into the domain enum (snake_case).
fn parse_kind(s: &str) -> Result<RequestKind, String> {
    match s {
        "question" => Ok(RequestKind::Question),
        "suggestion" => Ok(RequestKind::Suggestion),
        "concern" => Ok(RequestKind::Concern),
        "bug" => Ok(RequestKind::Bug),
        "feedback" => Ok(RequestKind::Feedback),
        _ => Err(format!(
            "invalid kind '{s}' (expected question|suggestion|concern|bug|feedback)"
        )),
    }
}

// ── Arg types ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CaptureArgs {
    /// One of: question, suggestion, concern, bug, feedback.
    #[serde(default)]
    #[schemars(required, !default)]
    pub kind: String,
    /// Who raised the request (display name / handle / email).
    #[serde(default)]
    pub from: String,
    /// The request content.
    #[serde(default)]
    #[schemars(required, !default)]
    pub body: String,
    /// PREFER THIS over packing detail into `body` — the structured body that
    /// renders as UI: {version:1, summary: one plain sentence (no shorthand,
    /// no ids), facts?: [{label,value}], sections?: [{heading, prose?:
    /// markdown, list?: [{text, done?}]}]}. `body` stays the plain-prose
    /// fallback.
    #[serde(default)]
    pub content: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetArgs {
    pub id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LinkArgs {
    pub id: String,
    /// A cross-ref string: `decision:N`, `step:X`, `progress:N`, `check:X`,
    /// `task:X`, `request:X`. `ref`/`crossref` accepted as forgiving aliases.
    #[serde(default, alias = "ref", alias = "crossref")]
    #[schemars(required, !default)]
    pub cross_ref: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PromoteArgs {
    pub id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ResearchArgs {
    pub id: String,
    /// A summary of what the agent found this round.
    #[serde(default)]
    #[schemars(required, !default)]
    pub summary: String,
    /// Agent confidence in the enrichment, 0..1 (clamped).
    #[serde(default)]
    pub confidence: f64,
    /// External references consulted (URLs, doc ids, ministr symbol ids).
    #[serde(default)]
    pub sources: Vec<String>,
    /// The decision_* step number that motivated this enrichment (cross-ref'd).
    #[serde(default)]
    pub note: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadyArgs {
    /// Minimum raising confidence (max enrichment confidence), 0..1. Default 0.6.
    #[serde(default)]
    pub min_confidence: Option<f64>,
    /// Relevance hints (active files / keywords). When given, only requests whose
    /// body or a cross-ref contains a hint are returned. Empty = no filter.
    #[serde(default)]
    pub hints: Vec<String>,
    /// Max requests to return. Default 20.
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RaiseArgs {
    pub id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SnoozeArgs {
    pub id: String,
    /// Minutes to suppress the request from request_ready. Default 60.
    #[serde(default)]
    pub minutes: Option<i64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DismissArgs {
    pub id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct NoArgs {}

// ── Tool handlers ──────────────────────────────────────────────────

#[tool_router]
impl RequestService {
    #[tool(
        name = "request_capture",
        description = "Capture a stakeholder request (question | suggestion | concern | bug | feedback) into the local store.\n\nInputs: kind (required — question|suggestion|concern|bug|feedback), from (who raised it), body (the content), content (RECOMMENDED — structured body: {version:1, summary, facts?[{label,value}], sections?[{heading, prose?, list?[{text,done?}]}]}; write the detail here so the webapp renders it as UI, and keep body to the person's plain words).\n\nReturns: the created request (id minted, status 'new', created timestamp).\n\nPitfalls: malformed content is rejected with the exact field named.\n\nNote: the local store is a cache of the cloud system-of-record; cloud write-through arrives with the SyncTarget::Cloud client.",
        annotations(
            title = "Capture request",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false,
        )
    )]
    pub async fn request_capture(
        &self,
        Parameters(args): Parameters<CaptureArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let kind = match parse_kind(&args.kind) {
            Ok(kind) => kind,
            Err(error) => return Ok(Self::err_structured("invalid_args", error)),
        };
        let content = match crate::content::parse_optional(args.content) {
            Ok(content) => content,
            Err(error) => return Ok(Self::err_structured("invalid_args", error)),
        };
        Self::present(
            self.application
                .capture(kind, args.from, args.body, content),
        )
    }

    #[tool(
        name = "request_status",
        description = "Request inbox snapshot: counts by lifecycle state (new/triaged/researched/raised/promoted/dismissed) + a newest-first, bounded list of request summaries.\n\nInputs: none.\n\nReturns: { project_id, counts, requests, omitted }.",
        annotations(
            title = "Request status",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn request_status(
        &self,
        Parameters(_): Parameters<NoArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::present(self.application.status())
    }

    #[tool(
        name = "request_get",
        description = "Fetch a single request by id, including its enrichment trail and cross-refs.\n\nInputs: id (required).\n\nReturns: the request, or a soft { ok:false, error_kind:'not_found' } when no such id.",
        annotations(
            title = "Get request",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn request_get(
        &self,
        Parameters(args): Parameters<GetArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::present(self.application.get(&args.id))
    }

    #[tool(
        name = "request_link",
        description = "Attach a typed cross-reference to a request, fusing it into the decision/run/plan graph.\n\nInputs: id (required), cross_ref (required — decision:N | step:X | progress:N | check:X | task:X | request:X).\n\nReturns: the updated request. Duplicates are ignored.",
        annotations(
            title = "Link request",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn request_link(
        &self,
        Parameters(args): Parameters<LinkArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::present(self.application.link(&args.id, &args.cross_ref))
    }

    #[tool(
        name = "request_promote",
        description = "Promote a validated (researched or raised) request into a plan idea — the opportunity→solution move. Writes bidirectional cross-refs: task:<id> onto the request, request:<id> onto the new task, so provenance runs both ways.\n\nInputs: id (required).\n\nReturns: { request_id, task_id, created, request/task }. Idempotent: a request already promoted returns its existing task with created=false (no duplicate).",
        annotations(
            title = "Promote request to task",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn request_promote(
        &self,
        Parameters(args): Parameters<PromoteArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::present(self.application.promote(&args.id))
    }

    #[tool(
        name = "request_research",
        description = "Record one round of churning on a request: append a durable enrichment { note?, sources[], summary, confidence } and advance the request's lifecycle toward 'researched' (new→triaged→researched). When note is given it is cross-ref'd onto the request so the reasoning is auditable.\n\nInputs: id (required), summary (required), confidence (0..1, clamped), sources (string[]), note (u32, optional).\n\nReturns: the updated request. Rejects a promoted/dismissed (terminal) request; never moves status backward.",
        annotations(
            title = "Research request",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false,
        )
    )]
    pub async fn request_research(
        &self,
        Parameters(args): Parameters<ResearchArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::present(self.application.research(
            &args.id,
            args.summary,
            args.confidence,
            args.sources,
            args.note,
        ))
    }

    #[tool(
        name = "request_ready",
        description = "Return requests ready to raise to the human under earned-interruption discipline: status 'researched', max-enrichment-confidence >= min_confidence, NOT already raised, NOT snoozed, and (when hints are given) relevant to the active context. Never returns an un-researched or low-confidence request.\n\nInputs: min_confidence (0..1, default 0.6), hints (string[] of active files/keywords — empty = no relevance filter), limit (default 20).\n\nReturns: { count, requests } (highest-confidence first).",
        annotations(
            title = "Ready requests",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn request_ready(
        &self,
        Parameters(args): Parameters<ReadyArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .application
            .ready(args.min_confidence, &args.hints, args.limit)
        {
            Ok(requests) => Self::present(Ok(serde_json::json!({
                "count": requests.len(),
                "requests": requests,
            }))),
            Err(error) => Self::present::<()>(Err(error)),
        }
    }

    #[tool(
        name = "request_raise",
        description = "Raise a request to the human: researched -> raised + stamps raised_at, so request_ready won't re-raise it.\n\nInputs: id (required).\n\nReturns: the updated request. Rejects a request that isn't researched.",
        annotations(
            title = "Raise request",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false,
        )
    )]
    pub async fn request_raise(
        &self,
        Parameters(args): Parameters<RaiseArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::present(self.application.raise(&args.id))
    }

    #[tool(
        name = "request_snooze",
        description = "Snooze a request: suppress it from request_ready for `minutes` (sets snooze_until = now + minutes). Status is unchanged.\n\nInputs: id (required), minutes (default 60).\n\nReturns: the updated request.",
        annotations(
            title = "Snooze request",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false,
        )
    )]
    pub async fn request_snooze(
        &self,
        Parameters(args): Parameters<SnoozeArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::present(self.application.snooze(&args.id, args.minutes))
    }

    #[tool(
        name = "request_dismiss",
        description = "Dismiss a request (terminal). Use when a raised request isn't worth acting on.\n\nInputs: id (required).\n\nReturns: the dismissed request.",
        annotations(
            title = "Dismiss request",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false,
        )
    )]
    pub async fn request_dismiss(
        &self,
        Parameters(args): Parameters<DismissArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::present(self.application.dismiss(&args.id))
    }
}

#[cfg(test)]
#[path = "handlers.tests.rs"]
mod tests;
