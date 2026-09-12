//! The `signal_*` tool handlers — rmcp `#[tool]` methods over [`SignalService`],
//! mirroring `roadmap::mcp::handlers`. Args deserialize forgivingly (absent
//! fields become defaults, never a -32602 that would cancel sibling calls);
//! logical failures return a soft `{ok:false, …}` envelope.

use rmcp::{
    ErrorData, handler::server::wrapper::Parameters, model::CallToolResult, tool, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;

use super::service::SignalService;
use crate::signal::domain::SignalKind;

impl SignalService {
    pub(crate) fn make_tool_router() -> rmcp::handler::server::router::tool::ToolRouter<Self> {
        Self::tool_router()
    }
}

/// Parse a wire kind string into the domain enum (snake_case).
fn parse_kind(s: &str) -> Result<SignalKind, String> {
    match s {
        "question" => Ok(SignalKind::Question),
        "idea" => Ok(SignalKind::Idea),
        "concern" => Ok(SignalKind::Concern),
        "bug" => Ok(SignalKind::Bug),
        "feedback" => Ok(SignalKind::Feedback),
        _ => Err(format!(
            "invalid kind '{s}' (expected question|idea|concern|bug|feedback)"
        )),
    }
}

// ── Arg types ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CaptureArgs {
    /// One of: question, idea, concern, bug, feedback.
    #[serde(default)]
    pub kind: String,
    /// Who raised the signal (display name / handle / email).
    #[serde(default)]
    pub from: String,
    /// The signal content.
    #[serde(default)]
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
    /// A cross-ref string: `think:N`, `task:X`, `action:N`, `check:X`,
    /// `chunk:X`, `signal:X`. `ref`/`crossref` accepted as forgiving aliases.
    #[serde(default, alias = "ref", alias = "crossref")]
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
    pub summary: String,
    /// Agent confidence in the enrichment, 0..1 (clamped).
    #[serde(default)]
    pub confidence: f64,
    /// External references consulted (URLs, doc ids, ministr symbol ids).
    #[serde(default)]
    pub sources: Vec<String>,
    /// The think_* step number that motivated this enrichment (cross-ref'd).
    #[serde(default)]
    pub think_step: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PendingArgs {
    /// Minimum surfacing confidence (max enrichment confidence), 0..1. Default 0.6.
    #[serde(default)]
    pub min_confidence: Option<f64>,
    /// Relevance hints (active files / keywords). When given, only signals whose
    /// body or a cross-ref contains a hint are returned. Empty = no filter.
    #[serde(default)]
    pub hints: Vec<String>,
    /// Max signals to return. Default 20.
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SurfaceArgs {
    pub id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SnoozeArgs {
    pub id: String,
    /// Minutes to suppress the signal from signal_pending. Default 60.
    #[serde(default)]
    pub minutes: Option<i64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct IgnoreArgs {
    pub id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct NoArgs {}

// ── Tool handlers ──────────────────────────────────────────────────

#[tool_router]
impl SignalService {
    #[tool(
        name = "signal_capture",
        description = "Capture a stakeholder signal (question | idea | concern | bug | feedback) into the local store.\n\nInputs: kind (required — question|idea|concern|bug|feedback), from (who raised it), body (the content), content (RECOMMENDED — structured body: {version:1, summary, facts?[{label,value}], sections?[{heading, prose?, list?[{text,done?}]}]}; write the detail here so the webapp renders it as UI, and keep body to the person's plain words).\n\nReturns: the created signal (id minted, status 'new', created timestamp).\n\nPitfalls: malformed content is rejected with the exact field named.\n\nNote: the local store is a cache of the cloud system-of-record; cloud write-through arrives with the SyncTarget::Cloud client.",
        annotations(
            title = "Capture signal",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false,
        )
    )]
    pub async fn signal_capture(
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
        name = "signal_status",
        description = "Signal inbox snapshot: counts by lifecycle state (new/triaged/researched/surfaced/promoted/dismissed) + a newest-first, bounded list of signal summaries.\n\nInputs: none.\n\nReturns: { project_id, counts, signals, omitted }.",
        annotations(
            title = "Signal status",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn signal_status(
        &self,
        Parameters(_): Parameters<NoArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::present(self.application.status())
    }

    #[tool(
        name = "signal_get",
        description = "Fetch a single signal by id, including its enrichment trail and cross-refs.\n\nInputs: id (required).\n\nReturns: the signal, or a soft { ok:false, error_kind:'not_found' } when no such id.",
        annotations(
            title = "Get signal",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn signal_get(
        &self,
        Parameters(args): Parameters<GetArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::present(self.application.get(&args.id))
    }

    #[tool(
        name = "signal_link",
        description = "Attach a typed cross-reference to a signal, fusing it into the think/ship/roadmap graph.\n\nInputs: id (required), cross_ref (required — think:N | task:X | action:N | check:X | chunk:X | signal:X).\n\nReturns: the updated signal. Duplicates are ignored.",
        annotations(
            title = "Link signal",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn signal_link(
        &self,
        Parameters(args): Parameters<LinkArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::present(self.application.link(&args.id, &args.cross_ref))
    }

    #[tool(
        name = "signal_promote",
        description = "Promote a validated (researched or surfaced) signal into a backlog roadmap chunk — the opportunity→solution move. Writes bidirectional cross-refs: chunk:<id> onto the signal, signal:<id> onto the new chunk, so provenance runs both ways.\n\nInputs: id (required).\n\nReturns: { signal_id, chunk_id, created, signal/chunk }. Idempotent: a signal already promoted returns its existing chunk with created=false (no duplicate).",
        annotations(
            title = "Promote signal to chunk",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn signal_promote(
        &self,
        Parameters(args): Parameters<PromoteArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        match self.application.promote(&args.id) {
            Ok(result) if result.created => Self::present(Ok(serde_json::json!({
                "signal_id": result.signal_id,
                "chunk_id": result.chunk_id,
                "created": true,
                "signal": result.signal,
            }))),
            Ok(result) => Self::present(Ok(serde_json::json!({
                "signal_id": result.signal_id,
                "chunk_id": result.chunk_id,
                "created": false,
                "chunk": result.chunk,
            }))),
            Err(error) => Self::present::<()>(Err(error)),
        }
    }

    #[tool(
        name = "signal_research",
        description = "Record one round of churning on a signal: append a durable enrichment { think_step?, sources[], summary, confidence } and advance the signal's lifecycle toward 'researched' (new→triaged→researched). When think_step is given it is cross-ref'd onto the signal so the reasoning is auditable.\n\nInputs: id (required), summary (required), confidence (0..1, clamped), sources (string[]), think_step (u32, optional).\n\nReturns: the updated signal. Rejects a promoted/dismissed (terminal) signal; never moves status backward.",
        annotations(
            title = "Research signal",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false,
        )
    )]
    pub async fn signal_research(
        &self,
        Parameters(args): Parameters<ResearchArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::present(self.application.research(
            &args.id,
            args.summary,
            args.confidence,
            args.sources,
            args.think_step,
        ))
    }

    #[tool(
        name = "signal_pending",
        description = "Return signals ready to raise to the human under earned-interruption discipline: status 'researched', max-enrichment-confidence >= min_confidence, NOT already surfaced, NOT snoozed, and (when hints are given) relevant to the active context. Never returns an un-researched or low-confidence signal.\n\nInputs: min_confidence (0..1, default 0.6), hints (string[] of active files/keywords — empty = no relevance filter), limit (default 20).\n\nReturns: { count, signals } (highest-confidence first).",
        annotations(
            title = "Pending signals",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn signal_pending(
        &self,
        Parameters(args): Parameters<PendingArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .application
            .pending(args.min_confidence, &args.hints, args.limit)
        {
            Ok(signals) => Self::present(Ok(serde_json::json!({
                "count": signals.len(),
                "signals": signals,
            }))),
            Err(error) => Self::present::<()>(Err(error)),
        }
    }

    #[tool(
        name = "signal_surface",
        description = "Mark a signal as surfaced (raised to the human): researched -> surfaced + stamps surfaced_at, so signal_pending won't re-raise it.\n\nInputs: id (required).\n\nReturns: the updated signal. Rejects a signal that isn't researched.",
        annotations(
            title = "Surface signal",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false,
        )
    )]
    pub async fn signal_surface(
        &self,
        Parameters(args): Parameters<SurfaceArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::present(self.application.surface(&args.id))
    }

    #[tool(
        name = "signal_snooze",
        description = "Snooze a signal: suppress it from signal_pending for `minutes` (sets snooze_until = now + minutes). Status is unchanged.\n\nInputs: id (required), minutes (default 60).\n\nReturns: the updated signal.",
        annotations(
            title = "Snooze signal",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false,
        )
    )]
    pub async fn signal_snooze(
        &self,
        Parameters(args): Parameters<SnoozeArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::present(self.application.snooze(&args.id, args.minutes))
    }

    #[tool(
        name = "signal_ignore",
        description = "Ignore a signal: dismiss it (terminal). Use when a surfaced signal isn't worth acting on.\n\nInputs: id (required).\n\nReturns: the dismissed signal.",
        annotations(
            title = "Ignore signal",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false,
        )
    )]
    pub async fn signal_ignore(
        &self,
        Parameters(args): Parameters<IgnoreArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::present(self.application.ignore(&args.id))
    }
}

#[cfg(test)]
#[path = "handlers.tests.rs"]
mod tests;
