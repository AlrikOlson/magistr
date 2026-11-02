//! `UnifiedService` — the single MCP server that exposes every tool family.
//!
//! Routes an incoming `tools/call` to the family that CLAIMS its name prefix:
//! [`DecisionService`], [`RunService`], [`PlanService`] or [`RequestService`].
//! Which of them a given deployment actually serves is decided once at startup
//! by [`FamilySelection`]; this layer only dispatches.
//!
//! A family may claim more than one prefix. [`Family::Plan`] claims
//! `tracker_*` as well as `plan_*`, because mirroring the plan into an issue
//! tracker moves plan state through a provider-agnostic port
//! ([`crate::tracker`]) rather than being a family of its own.
//!
//! [`Family::ALL`] and [`Family::prefixes`] are the one table of families and
//! the namespaces they claim, and everything that describes or dispatches the
//! surface derives from them rather than restating them — routing, the
//! operator-typed family selection, the `initialize` block, and the crate's own
//! front page, each held to it by a gate below.

use std::sync::{Arc, Mutex};

use rmcp::{
    ErrorData, RoleServer, ServerHandler,
    model::{
        CallToolRequestParams, CallToolResponse, CancelTaskParams, GetTaskParams, GetTaskResult,
        Implementation, ListResourceTemplatesResult, ListResourcesResult, ListToolsResult,
        PaginatedRequestParams, ReadResourceRequestParams, ReadResourceResponse,
        ReadResourceResult, Resource, ResourceContents, ResourceTemplate, ServerCapabilities,
        ServerConfig, UpdateTaskParams,
    },
    service::RequestContext,
    task_manager::TaskManager,
};

use super::cache::{catalog, live_state};
use super::progress::Heartbeat;
use super::resources::{
    DIGEST_DEFAULT_URI, DIGEST_PREFIX, MARKDOWN, PINNED_URI, PLAN_URI, digest_markdown,
    parse_window, pinned_markdown,
};
use super::tasks::{Eligibility, HeartbeatSeed};

use crate::decision::DecisionService;
use crate::plan::PlanService;
use crate::request::RequestService;
use crate::run::RunService;
use crate::usage::CallCounter;

const SERVER_INSTRUCTIONS: &str = r#"magistr — unified MCP server for plan-driven reasoning + execution.

Four tool families share one server. plan_* claims a second prefix, tracker_*,
for the namespace it mirrors itself through:

  plan_* (17 tools)   the long-horizon plan-of-plans (drives the project)
  tracker_* (2 tools)    plan's mirroring namespace — project the plan into an
                         issue tracker through one provider-agnostic port
  decision_*   (13 tools)   decision log
  run_*      (7 tools)  run history: open a run, read its status, finish it
  approval_* (2 tools)  the run family's approvals: ask a person, wait for the answer
  step verbs (5 tools)  arrange_steps, start_step, record_progress, check_step, finish_step
  request_*  (10 tools)   stakeholder requests (capture/research/surface/promote + inbox verbs)

plan_* sits above run_*: a plan task is realized by a run, which is
motivated by decision reasoning. Link reasoning to a run through run_ref on
decision_record (e.g. "step:auth-refactor"), and progress back to reasoning
through the note field on record_progress. All families resolve the same
project identity from the working directory so their records correlate.
"#;

/// Single MCP server exposing the `decision_*`, `run_*`, `plan_*`, and
/// `request_*` families.
#[derive(Clone)]
pub struct UnifiedService {
    decision: Arc<DecisionService>,
    run: Arc<RunService>,
    plan: Arc<PlanService>,
    request: Arc<RequestService>,
    /// The last `traceparent` written to the otel partition. A tool call is a
    /// hot path and the adopted context is usually unchanged from the previous
    /// one, so this collapses a per-call file write into a per-trace one.
    last_traceparent: Arc<Mutex<Option<String>>>,
    /// SEP-2663 task store. Cheaply cloneable and shared across clones of this
    /// service, so the `tasks/get` that polls a handle reaches the same store
    /// the `tools/call` that created it wrote to.
    tasks: TaskManager,
    /// Which families this deployment exposes. Resolved once at startup and
    /// identical for every connection — see [`FamilySelection`].
    families: FamilySelection,
    /// Per-tool invocation counts. Local, never transmitted, on unless
    /// switched off — the argument is in [`crate::usage`].
    calls: Arc<CallCounter>,
    /// The LIVE OTLP lane. Disabled — no thread, no network — unless an OTLP
    /// endpoint is configured; see [`crate::otel_live`] for why endpoint
    /// presence is the switch and why emission can never delay a call.
    live: crate::otel_live::LiveEmitter,
}

impl UnifiedService {
    pub fn new(
        decision: DecisionService,
        run: RunService,
        plan: PlanService,
        request: RequestService,
    ) -> Self {
        // Every tool name this binary registers, taken from the families
        // themselves rather than a hand-written list — and BEFORE any family
        // selection is applied, because a deselected family's tool is still a
        // real name and a refused call is still a call worth counting.
        let known: Vec<String> = decision
            .list_tools_view()
            .into_iter()
            .chain(run.list_tools_view())
            .chain(plan.list_tools_view())
            .chain(request.list_tools_view())
            .map(|t| t.name.to_string())
            .collect();
        Self {
            decision: Arc::new(decision),
            run: Arc::new(run),
            plan: Arc::new(plan),
            request: Arc::new(request),
            last_traceparent: Arc::new(Mutex::new(None)),
            tasks: TaskManager::new(),
            families: FamilySelection::all(),
            calls: Arc::new(CallCounter::from_env(known)),
            live: crate::otel_live::LiveEmitter::from_env(&crate::infra::resolve_project_id(None)),
        }
    }

    /// Drain BOTH live OTLP lanes and publish the workspace span that contains
    /// this session. Idempotent, bounded, and a no-op when a lane is off, so a
    /// shutdown path can call it unconditionally.
    ///
    /// The log lane is flushed here for a reason found by running it rather than
    /// by reading it: its worker only POSTs on a 500ms batch timer, and a stdio
    /// session ends the instant the client closes the pipe. Without this line the
    /// last warnings of a session — which are exactly the ones an operator went
    /// looking for — die with the process. The first version shipped that bug and
    /// `SELECT count() FROM otel_logs` stayed at 0.
    pub fn flush_telemetry(&self) {
        self.live.flush();
        crate::otel_logs::flush();
    }

    /// Declare which transport this process serves on, for the live lane's
    /// `network.transport` attribute. Called once by `run_stdio` / `run_http`,
    /// which are the only places that know.
    pub fn set_transport(&self, transport: &'static str) {
        self.live.set_transport(transport);
    }

    /// Replace the invocation counter — the seam a test uses to point counting
    /// at a scratch directory without mutating process-global environment.
    #[must_use]
    pub fn with_call_counter(mut self, calls: CallCounter) -> Self {
        self.calls = Arc::new(calls);
        self
    }

    /// This deployment's invocation counts, as persisted.
    #[must_use]
    pub fn call_counts(&self) -> crate::usage::CallCounts {
        self.calls.snapshot()
    }

    /// Restrict this deployment to `families`. Consuming builder so the
    /// selection is fixed at construction and cannot drift per request.
    #[must_use]
    pub fn with_families(mut self, families: FamilySelection) -> Self {
        self.families = families;
        self
    }

    /// The families this deployment exposes.
    #[must_use]
    pub fn families(&self) -> FamilySelection {
        self.families
    }

    /// The SEP-2663 task store backing `tasks/get`, `tasks/update` and
    /// `tasks/cancel`. Exposed so a test can observe a task's terminal payload
    /// without going through the wire.
    #[must_use]
    pub fn task_manager(&self) -> &TaskManager {
        &self.tasks
    }

    /// Dispatch a tool call to its family. **The only implementation.**
    ///
    /// The inline path awaits this directly and the task path awaits this
    /// inside a spawned future — so "a gate run as a task and the same gate run
    /// inline produce the same envelope" is not a property to be reviewed for,
    /// it is the absence of a second implementation. `verified`, `exit_code`
    /// and the recorded check all come from the one `check_step` handler either
    /// way.
    async fn route(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        // Count FIRST — before the family guard, before dispatch, and without
        // looking at the outcome. A call that errors is a call: a verb that is
        // always misused is data, and a verb this deployment refuses is exactly
        // the datum a family-selection decision needs. Counting anywhere after
        // this point would silently under-report the failures, which is the
        // same self-flattering bias that made `tools_used` useless
        // (see [`crate::usage`]).
        self.calls
            .record(&request.name, &chrono::Utc::now().to_rfc3339());

        // Dispatch must agree with `list_tools_view`. A deselected family is
        // not listed, so reaching here means the caller guessed a name or is
        // replaying an older session — either way it gets told the surface was
        // narrowed on purpose, and how to widen it, rather than "unknown tool".
        if let Some(family) = Self::route_of(&request.name)
            && !self.families.contains(family)
        {
            return Err(ErrorData::invalid_params(
                format!(
                    "tool '{}' belongs to the {}_* family, which this deployment does not expose \
                     (this server serves: {}). Set {FAMILIES_ENV} to include '{}', or unset it \
                     for all families.",
                    request.name,
                    family.prefix(),
                    self.families.summary(),
                    family.prefix(),
                ),
                None,
            ));
        }

        // MEASURED, not reconstructed. This instant and the one taken after the
        // await are the live lane's entire claim over the offline export, whose
        // durations are inferred from stored record timestamps.
        //
        // The identifying facts are captured HERE because `context` is moved
        // into the dispatch below and is gone by the time the span is built.
        let tool = request.name.to_string();
        let request_id = context.id.to_string();
        let protocol_version = context.protocol_version().map(|v| v.to_string());
        let client = context.client_info();
        let started = crate::otel_live::now_nanos();

        // The ids the span for THIS call will carry, computed before it runs.
        // The span object cannot exist yet — it has not ended — but its id is a
        // pure function of (project, tool, start), so the correlating pair is
        // knowable here and is published for the duration of the dispatch. A
        // `tracing::warn!` anywhere inside then reaches the backend already
        // pointing at the span that contains it, which is the one thing a
        // logs-and-traces platform does that Jaeger cannot.
        let correlation = self.live.correlation(&tool, started);
        let dispatch = async {
            let outcome = match Self::route_of(&request.name) {
                Some(Family::Decision) => self.decision.call_tool(request, context).await,
                Some(Family::Run) => self.run.call_tool(request, context).await,
                Some(Family::Plan) => self.plan.call_tool(request, context).await,
                Some(Family::Request) => self.request.call_tool(request, context).await,
                None => Err(ErrorData::invalid_params(
                    format!(
                        "unknown tool '{}'; expected a decision_*, run_*, plan_*, or request_* \
                         name",
                        request.name
                    ),
                    None,
                )),
            };
            // Logged INSIDE the scope, which is the only place it can be: this
            // is the failure an operator searches the log view for, and it is
            // the click-through the span lane cannot provide on its own — the
            // span carries the JSON-RPC code, this carries the sentence.
            if let Err(e) = &outcome {
                tracing::warn!(
                    target: "magistr_workflow::mcp",
                    "tool call failed: {} ({}): {}",
                    tool,
                    e.code.0,
                    e.message
                );
            }
            outcome
        };
        let outcome = crate::otel_logs::in_call(correlation, dispatch).await;
        // AFTER the call, never around it: a span is only emittable once it has
        // ended, and the enqueue is non-blocking so this cannot lengthen the
        // call it describes. A failed call is still a span — with the JSON-RPC
        // code as `error.type`, which is what makes it an ERROR in the backend
        // rather than indistinguishable from success. Same reason
        // `calls.record` counts failures.
        let failure = outcome.as_ref().err();
        self.live.tool_call(&crate::otel_live::ToolCall {
            tool: &tool,
            start_nanos: started,
            end_nanos: crate::otel_live::now_nanos(),
            request_id: Some(&request_id),
            protocol_version: protocol_version.as_deref(),
            client_name: client.as_ref().map(|c| c.name.as_str()),
            client_version: client.as_ref().map(|c| c.version.as_str()),
            error_code: failure.map(|e| e.code.0),
            error_message: failure.map(|e| e.message.as_ref()),
        });
        outcome
    }

    /// Adopt a caller's W3C Trace Context from a request's `_meta` (SEP-414),
    /// persisting it so `magistr trace export` — a later, separate
    /// process — can parent our trace to the caller's span.
    ///
    /// Every failure mode here is silence. A missing, malformed, or
    /// unpersistable context leaves the export exactly as it was; nothing about
    /// observability is worth failing a tool call over.
    fn adopt_trace_context(&self, meta: &rmcp::model::RequestMetaObject) {
        let Some(raw) = meta.get_traceparent() else {
            return;
        };
        // Unchanged context: already stored, nothing to write.
        {
            let Ok(last) = self.last_traceparent.lock() else {
                return;
            };
            if last.as_deref() == Some(raw) {
                return;
            }
        }
        let Some(adopted) = crate::trace_context::InboundTrace::from_meta(
            Some(raw),
            meta.get_tracestate(),
            meta.get_baggage(),
            chrono::Utc::now().to_rfc3339(),
        ) else {
            return;
        };
        let project = crate::infra::resolve_project_id(None);
        if crate::trace_context::store(&project, &adopted).is_ok()
            && let Ok(mut last) = self.last_traceparent.lock()
        {
            *last = Some(raw.to_string());
        }
        // Re-point the LIVE lane at the caller's trace. This is the only moment
        // the right answer is knowable: the emitter is built at startup, and
        // the context arrives on a call. Without this the live spans would sit
        // in a trace of our own minting while the export joined the caller's —
        // two lanes describing two unrelated trees.
        self.live
            .rebind(&adopted.trace_id, Some(&adopted.parent_span_id));
    }

    /// Returns the combined `tools/list` view across every selected family.
    /// Deselected families contribute nothing — which is the whole point, and
    /// is also why `route` must refuse them: listing and dispatch have to agree
    /// or a tool becomes advertised-but-uncallable (the `tracker_*` bug that
    /// [`Self::route_of`] documents).
    pub fn list_tools_view(&self) -> Vec<rmcp::model::Tool> {
        let mut tools = Vec::new();
        if self.families.contains(Family::Decision) {
            tools.extend(self.decision.list_tools_view());
        }
        if self.families.contains(Family::Run) {
            tools.extend(self.run.list_tools_view());
        }
        if self.families.contains(Family::Plan) {
            tools.extend(self.plan.list_tools_view());
        }
        if self.families.contains(Family::Request) {
            tools.extend(self.request.list_tools_view());
        }
        tools
    }

    /// Which underlying family a tool name routes to. Returns `None` for
    /// names that no family claims.
    ///
    /// A DERIVATION over [`Family::prefixes`], never a branch per prefix. That
    /// distinction is the whole fix for the bug this function used to carry:
    /// `tools/list` advertises whatever the families register, but dispatch is
    /// by prefix, so a prefix nothing here NAMED was invisible — a tool could be
    /// listed and still answer "unknown tool", which is exactly what
    /// `tracker_setup` did on its first real handshake. Reading the claim off
    /// the family table instead of restating it means a new prefix is routable
    /// the moment it is claimed, and `every_listed_tool_can_actually_be_routed`
    /// holds the two surfaces together.
    pub fn route_of(name: &str) -> Option<Family> {
        Family::ALL.into_iter().find(|family| family.claims(name))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    Decision,
    Run,
    Plan,
    Request,
}

impl Family {
    /// Every family this server serves. Callers that describe the surface —
    /// the instructions block, the generated CLAUDE.md — iterate this instead
    /// of hand-listing families, so adding a fifth can't leave a doc behind.
    pub const ALL: [Family; 4] = [Family::Decision, Family::Run, Family::Plan, Family::Request];

    /// Every tool-name prefix this family CLAIMS, without trailing
    /// underscores, canonical one first.
    ///
    /// This is the single table the whole prefix story is read from — routing,
    /// operator-typed family tokens, the refusal that lists known families, and
    /// the gate on the `initialize` block. A prefix that appears anywhere on the
    /// surface and not here is the defect those callers exist to catch.
    ///
    /// # Why a family may claim more than one
    ///
    /// `tracker_*` is the mirroring namespace of [`Family::Plan`], not a
    /// fifth family. The subsystem behind it is large — a provider-agnostic port
    /// with adapters for GitHub Issues, GitHub Projects and Linear — but the
    /// state it moves is plan state: the opt-ins live on the `Plan` and
    /// the handlers hang off `PlanService`, so the family that owns the plan
    /// owns the namespace that mirrors it. Giving that fact a slot in the table
    /// is what lets every caller derive from it rather than special-case it.
    pub fn prefixes(self) -> &'static [&'static str] {
        match self {
            Family::Decision => &["decision"],
            Family::Run => &["run", "approval"],
            Family::Plan => &["plan", "tracker"],
            Family::Request => &["request"],
        }
    }

    /// The prefix that NAMES the family — the head of its claim.
    ///
    /// Falls back to the empty string rather than unwrapping, so a table entry
    /// emptied by accident cannot panic a live server; `every_family_claims_at
    /// _least_one_prefix` is what actually forbids that state.
    pub fn prefix(self) -> &'static str {
        self.prefixes().first().copied().unwrap_or_default()
    }

    /// The prefixes this family claims BEYOND the one that names it.
    pub fn aliases(self) -> &'static [&'static str] {
        self.prefixes()
            .split_first()
            .map_or(&[][..], |(_, rest)| rest)
    }

    /// The direct verbs this family serves under their own names. A step
    /// verb names what it acts on (`start_step`, `check_step`), not the family,
    /// so it is claimed by exact name rather than by prefix.
    pub fn verbs(self) -> &'static [&'static str] {
        match self {
            Family::Run => &crate::run::mcp::STEP_VERBS,
            Family::Decision | Family::Plan | Family::Request => &[],
        }
    }

    /// Whether `tool_name` sits under one of this family's prefixes or is
    /// one of its direct verbs.
    pub fn claims(self, tool_name: &str) -> bool {
        self.verbs().contains(&tool_name)
            || self.prefixes().iter().any(|p| {
                tool_name
                    .strip_prefix(p)
                    .is_some_and(|v| v.starts_with('_'))
            })
    }

    /// Parse an operator-written family token (case- and space-insensitive).
    ///
    /// Any prefix a family claims is a spelling of it, so an operator who writes
    /// `tracker` reaches [`Family::Plan`] — the same structural fact
    /// [`UnifiedService::route_of`] derives, read from the same table rather
    /// than restated where a human types it.
    pub fn parse(token: &str) -> Option<Family> {
        let token = token.trim().trim_end_matches('_').to_ascii_lowercase();
        Family::ALL
            .into_iter()
            .find(|f| f.prefixes().contains(&token.as_str()))
    }

    /// Every family and its aliases, as the one sentence that advertises the
    /// set. Derived, so a refusal cannot word the list differently from the
    /// table it is refusing against.
    fn known_list() -> String {
        Family::ALL
            .iter()
            .map(|f| {
                if f.aliases().is_empty() {
                    f.prefix().to_string()
                } else {
                    format!("{} (aka {})", f.prefix(), f.aliases().join(", "))
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Selects which tool families this **deployment** exposes.
///
/// # Why this is per-deployment and not per-connection
///
/// The 2026-07-28 core is stateless: `tools/list` no longer varies per
/// connection, so a server may not decide its surface from who is asking.
/// SEP-1300 ("Tool Filtering with Groups and Tags") would give the *client* a
/// say, but it is an open SEP and pre-empting it with a private protocol would
/// be inventing wire semantics. What is left — and what this is — is a choice
/// the operator makes before the process starts, identical for every
/// connection it then serves.
///
/// Resolved once in `build_unified` and never consulted again per request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FamilySelection {
    decision: bool,
    run: bool,
    plan: bool,
    request: bool,
}

/// Why a family selection was refused. Every variant is a startup error: a
/// misspelling must not quietly remove tools someone depends on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FamilySelectionError {
    /// A token that names no family.
    Unknown { token: String },
    /// A selection that would expose no tools at all.
    Empty,
}

impl std::fmt::Display for FamilySelectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let known = Family::known_list();
        match self {
            Self::Unknown { token } => write!(
                f,
                "unknown tool family '{token}' in {FAMILIES_ENV}; known families are {known}"
            ),
            Self::Empty => write!(
                f,
                "{FAMILIES_ENV} selected no tool families; a server with no tools cannot do \
                 anything. Unset it for all families, or name at least one of {known}"
            ),
        }
    }
}

impl std::error::Error for FamilySelectionError {}

/// Comma-separated list of families this deployment exposes. Unset = all.
pub const FAMILIES_ENV: &str = "MAGISTR_WORKFLOW_FAMILIES";

/// Tools that legitimately have NO `outputSchema`, with the reason.
///
/// One entry, and it is a real exemption rather than a gap: `decision_export`
/// returns markdown, console text or JSON depending on the `format` argument, so
/// there is no single shape to declare. Declaring one would be a lie a client
/// could validate against.
pub const SCHEMA_EXEMPT: &[(&str, &str)] = &[
    (
        "decision_export",
        "output shape is format-dependent (markdown | console | json); no single schema can describe it",
    ),
    (
        "plan_focus_get",
        "a schema is describable but not affordable: measured at +1,096 B for the pair even with both rich fields as bare Values, against 370 B of headroom under a ceiling a human had just moved for this task. Shapes are documented in the description instead. Payable once the duplicated request_* schemas are reclaimed — see plan::output_schemas",
    ),
    (
        "plan_focus_set",
        "same measured budget refusal as plan_focus_get; the pair shares one response shape",
    ),
];

/// Tools that SHOULD carry an `outputSchema` and do not — a known gap, not a
/// design choice, pinned here so absence stops being ambiguous.
///
/// # Empty, and it took a deliberate decision to get there
///
/// This list briefly held fourteen names. Writing their schemas took coverage
/// to 47/48 and `tools/list` from 97,199 B to 135,524 B — 35% through the
/// 100,000 B ceiling `tools_list_payload_stays_within_budget` installed in
/// f275f10, whose attached rule was "re-decide the surface, don't raise the
/// line". That re-decision was taken by a human rather than assumed, and it
/// went in favour of the schemas; the ceiling moved to 140,000 with the
/// argument recorded on the test itself.
///
/// The cost was structural, not sloppiness, and it is worth knowing before
/// anyone adds a fifteenth request tool: seven `request_*` tools return a bare
/// `Request`, whose schema is 3,576 B once its nested `Enrichment` /
/// `RequestKind` / `RequestStatus` definitions expand — and MCP has no way to
/// `$ref` a schema across tools, so that is seven full copies, ~25 KB for one
/// type. Adding a type to a shared response shape is therefore multiplied by
/// however many tools return it.
///
/// **This list must stay empty.** A new tool ships with a schema, or with a
/// declared entry in [`SCHEMA_EXEMPT`] and a reason. A tool in neither fails
/// `every_tool_is_dispositioned_for_output_schema`.
pub const SCHEMA_PENDING_BUDGET: &[&str] = &[];

impl Default for FamilySelection {
    fn default() -> Self {
        Self::all()
    }
}

impl FamilySelection {
    /// Every family — the default, and what an install that sets nothing gets.
    #[must_use]
    pub const fn all() -> Self {
        Self {
            decision: true,
            run: true,
            plan: true,
            request: true,
        }
    }

    /// Parse a comma-separated selection. Empty/whitespace tokens are skipped
    /// so a trailing comma is not an error; a token that names nothing is.
    pub fn parse(raw: &str) -> Result<Self, FamilySelectionError> {
        let mut sel = Self {
            decision: false,
            run: false,
            plan: false,
            request: false,
        };
        for token in raw.split(',').map(str::trim).filter(|t| !t.is_empty()) {
            match Family::parse(token) {
                Some(f) => sel.set(f),
                None => {
                    return Err(FamilySelectionError::Unknown {
                        token: token.to_string(),
                    });
                }
            }
        }
        if sel.selected().next().is_none() {
            return Err(FamilySelectionError::Empty);
        }
        Ok(sel)
    }

    /// Resolve from the environment. Absent or entirely-blank = every family,
    /// so an existing install that sets nothing is byte-identical.
    pub fn from_env() -> Result<Self, FamilySelectionError> {
        match std::env::var(FAMILIES_ENV) {
            Ok(raw) if !raw.trim().is_empty() => Self::parse(&raw),
            _ => Ok(Self::all()),
        }
    }

    fn set(&mut self, family: Family) {
        match family {
            Family::Decision => self.decision = true,
            Family::Run => self.run = true,
            Family::Plan => self.plan = true,
            Family::Request => self.request = true,
        }
    }

    /// Is this family exposed by this deployment?
    #[must_use]
    pub fn contains(self, family: Family) -> bool {
        match family {
            Family::Decision => self.decision,
            Family::Run => self.run,
            Family::Plan => self.plan,
            Family::Request => self.request,
        }
    }

    /// Is every family exposed? (The default — used to stay silent when there
    /// is nothing unusual to announce.)
    #[must_use]
    pub fn is_all(self) -> bool {
        self == Self::all()
    }

    /// The selected families, in [`Family::ALL`] order.
    pub fn selected(self) -> impl Iterator<Item = Family> {
        Family::ALL.into_iter().filter(move |f| self.contains(*f))
    }

    /// Human-readable summary for `doctor` and the startup announcement.
    #[must_use]
    pub fn summary(self) -> String {
        self.selected()
            .map(|f| format!("{}_*", f.prefix()))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

impl UnifiedService {
    /// The fixed resource catalog (mcp-resources). The digest entry is the
    /// 24h default; arbitrary windows come via the template.
    fn resource_catalog() -> Vec<Resource> {
        let entry = |uri: &str, name: &str, description: &str| {
            Resource::new(uri, name)
                .with_description(description)
                .with_mime_type(MARKDOWN)
        };
        vec![
            entry(
                PLAN_URI,
                "Plan",
                "The project plan (generated markdown view of native task state).",
            ),
            entry(
                PINNED_URI,
                "Pinned decisions",
                "Pinned decision notes — the trace's durable, load-bearing conclusions.",
            ),
            entry(
                DIGEST_DEFAULT_URI,
                "Digest (last 24h)",
                "Recent activity: reasoning steps recorded and plan tasks that moved.",
            ),
        ]
    }

    /// Returns the MRTR outcome (SEP-2322) rather than a bare
    /// `ReadResourceResult`: every resource this server serves is read from
    /// local state and completes in one round trip, so it is always
    /// `Complete`. Wrapping here keeps the three call sites unchanged — which
    /// is also why the SEP-2549 stamp goes here: one seam covers all three
    /// resources, and a fourth cannot be added without passing through it.
    fn text_resource(uri: &str, text: String) -> ReadResourceResponse {
        live_state(ReadResourceResult::new(vec![
            ResourceContents::TextResourceContents {
                uri: uri.to_string(),
                mime_type: Some(MARKDOWN.into()),
                text,
                meta: None,
            },
        ]))
        .into()
    }

    fn lock_poisoned() -> ErrorData {
        ErrorData::internal_error("engine lock poisoned", None)
    }
}

impl ServerHandler for UnifiedService {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                // Read-only resources only — no subscriptions until the
                // 2026-07-28 spec finalizes its stateless core.
                .enable_resources()
                // SEP-2663. Declaring it is what makes `tasks/get` reachable at
                // all: rmcp answers the three tasks/* methods with -32601 unless
                // the SERVER advertised the extension, so a handle we returned
                // would be one the client could never poll. It costs nothing in
                // `tools/list` — the extension bag rides the `initialize`
                // result — and no client is forced to use it.
                .enable_tasks()
                .build(),
        )
        .with_instructions(SERVER_INSTRUCTIONS)
        .with_server_info(Implementation::new(
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION"),
        ))
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(catalog(ListToolsResult::with_all_items(
            self.list_tools_view(),
        )))
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        Ok(catalog(ListResourcesResult::with_all_items(
            Self::resource_catalog(),
        )))
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, ErrorData> {
        let template = ResourceTemplate::new(format!("{DIGEST_PREFIX}{{window}}"), "Digest")
            .with_description(
                "Recent activity since now-window; window is <n>h or <n>d (e.g. 24h, 7d).",
            )
            .with_mime_type(MARKDOWN);
        Ok(catalog(ListResourceTemplatesResult::with_all_items(vec![
            template,
        ])))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, ErrorData> {
        let uri = request.uri.as_str();
        if uri == PLAN_URI {
            let engine = self.plan.engine();
            let mut guard = engine.lock().map_err(|_| Self::lock_poisoned())?;
            guard
                .refresh_if_stale()
                .map_err(|error| ErrorData::internal_error(error, None))?;
            return Ok(Self::text_resource(uri, guard.export("markdown")));
        }
        if uri == PINNED_URI {
            let engine = self.decision.engine();
            let guard = engine.lock().map_err(|_| Self::lock_poisoned())?;
            return Ok(Self::text_resource(
                uri,
                pinned_markdown(&guard.history().notes),
            ));
        }
        if let Some(spec) = uri.strip_prefix(DIGEST_PREFIX) {
            let window = parse_window(spec).ok_or_else(|| {
                ErrorData::invalid_params(
                    format!("invalid digest window '{spec}'; expected <n>h or <n>d (e.g. 24h, 7d)"),
                    None,
                )
            })?;
            let steps = {
                let engine = self.decision.engine();
                let guard = engine.lock().map_err(|_| Self::lock_poisoned())?;
                guard.history().notes.clone()
            };
            let tasks = {
                let engine = self.plan.engine();
                let mut guard = engine.lock().map_err(|_| Self::lock_poisoned())?;
                guard
                    .refresh_if_stale()
                    .map_err(|error| ErrorData::internal_error(error, None))?;
                guard.plan().tasks.clone()
            };
            return Ok(Self::text_resource(
                uri,
                digest_markdown(&steps, &tasks, chrono::Utc::now(), window),
            ));
        }
        Err(ErrorData::resource_not_found(
            format!(
                "unknown resource '{uri}'; available: {PLAN_URI}, {PINNED_URI}, {DIGEST_PREFIX}<n>h|<n>d"
            ),
            None,
        ))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        // SEP-414: rmcp lifts the wire `params._meta` into `context.meta`
        // before dispatch, so this one seam covers every tool in all five
        // families.
        self.adopt_trace_context(&context.meta);
        // Liveness progress rides the SAME seam as the trace context above:
        // one place covers every tool in every family, so `check_step`'s gate,
        // a tracker push and a sync back-fill all report without any of them
        // knowing this exists. The handle is held only for the duration of the
        // inner call and aborts on drop, so a call that finishes inside
        // `progress::FIRST_TICK` emits nothing at all.
        //
        // It is deliberately started AFTER the retired-name check: that branch
        // returns immediately and has nothing to be alive about.
        //
        // SEP-2663 rides the same seam, and takes its branch BEFORE the
        // heartbeat above is started. That ordering is the whole of the "no
        // second progress mechanism" rule `progress.rs` set down: a task-backed
        // call starts its one heartbeat inside the spawned future — where the
        // task id finally exists — so no call can ever own two.
        if let Some(eligible) = Eligibility::decide(
            &self.tasks,
            &context,
            request.name.as_ref(),
            request.arguments.as_ref(),
        ) {
            let seed =
                HeartbeatSeed::new(context.peer.clone(), &context.meta, request.name.as_ref());
            let svc = self.clone();
            return Ok(eligible.spawn(
                seed,
                move || async move { svc.route(request, context).await },
            ));
        }
        let _heartbeat =
            Heartbeat::start(context.peer.clone(), &context.meta, request.name.as_ref());
        self.route(request, context).await
    }

    /// SEP-2663 `tasks/get`. rmcp has already refused the call unless both this
    /// server and the client declared the extension.
    async fn get_task(
        &self,
        request: GetTaskParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetTaskResult, ErrorData> {
        self.tasks
            .get_task(&request.task_id)
            .map(GetTaskResult::new)
    }

    async fn update_task(
        &self,
        request: UpdateTaskParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<(), ErrorData> {
        self.tasks
            .update_task(&request.task_id, request.input_responses)
    }

    async fn cancel_task(
        &self,
        request: CancelTaskParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<(), ErrorData> {
        self.tasks.cancel_task(&request.task_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn service() -> UnifiedService {
        let run = RunService::new(crate::run::engine::RunEngine::new("p".into()));
        let plan = PlanService::new(
            crate::plan::engine::PlanEngine::new("p".into()),
            run.use_cases(),
        );
        UnifiedService::new(
            DecisionService::new(crate::decision::engine::core::ReasoningServer::new(
                crate::decision::config::DecisionConfig::default(),
            )),
            run,
            plan,
            RequestService::new(crate::request::engine::RequestEngine::new("p".into())),
        )
    }

    /// Every `<stem>_*` token a block of prose advertises as a tool namespace.
    ///
    /// Takes the text as a PARAMETER so the gate below can be driven against a
    /// block this server does not serve. A gate that can only ever read the one
    /// real string proves the string, never the check.
    fn advertised_prefixes(text: &str) -> Vec<&str> {
        text.split_whitespace()
            .map(|w| w.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '*'))
            .filter_map(|w| w.strip_suffix("_*"))
            .filter(|stem| !stem.is_empty())
            .collect()
    }

    #[test]
    fn route_of_recognizes_all_four_prefixes() {
        assert_eq!(
            UnifiedService::route_of("decision_record"),
            Some(Family::Decision)
        );
        assert_eq!(UnifiedService::route_of("run_open"), Some(Family::Run));
        assert_eq!(
            UnifiedService::route_of("plan_add_task"),
            Some(Family::Plan)
        );
        assert_eq!(
            UnifiedService::route_of("request_capture"),
            Some(Family::Request)
        );
        assert_eq!(
            UnifiedService::route_of("tracker_setup"),
            Some(Family::Plan),
            "tracker_* is the Plan family's mirroring namespace, because the \
             opt-ins it moves are plan state"
        );
    }

    /// THE GAP THIS CLOSES, and it is a shape rather than one missing arm.
    ///
    /// `tools/list` advertises whatever the families registered, but `call_tool`
    /// dispatches by PREFIX. Those are two different sources of truth, so a tool
    /// can be listed and yet answer "unknown tool" — which is exactly what
    /// `tracker_setup` did on its first real handshake. Every unit test invoked
    /// the handler directly and never touched the router, so nothing caught it.
    ///
    /// This walks the ACTUAL served list and demands each name route somewhere.
    /// Adding a family with a new prefix and forgetting the router now fails
    /// here instead of at a user's first call.
    #[test]
    fn every_listed_tool_can_actually_be_routed() {
        let svc = service();
        let listed: Vec<String> = svc
            .list_tools_view()
            .into_iter()
            .map(|t| t.name.to_string())
            .collect();
        assert!(
            listed.len() >= 40,
            "the served list looks wrong ({} tools), so this proves nothing",
            listed.len()
        );
        let orphans: Vec<&String> = listed
            .iter()
            .filter(|n| UnifiedService::route_of(n).is_none())
            .collect();
        assert!(
            orphans.is_empty(),
            "these tools are ADVERTISED but cannot be called — tools/list and \
             call_tool disagree, so a client sees them and gets 'unknown tool': \
             {orphans:?}"
        );
    }

    /// The step verbs carry no family prefix, so the run family claims them
    /// by exact name. Retired names are not claimed by anyone and are refused
    /// as unknown tools.
    #[test]
    fn step_verbs_route_to_the_run_family_and_retired_names_route_nowhere() {
        for verb in crate::run::mcp::STEP_VERBS {
            assert_eq!(UnifiedService::route_of(verb), Some(Family::Run), "{verb}");
        }
        assert_eq!(UnifiedService::route_of("approval_open"), Some(Family::Run));
        for retired in [
            "ship_status",
            "ship_check",
            "check_work",
            "ship_ship",
            "roadmap_status",
            "roadmap_add_chunk",
        ] {
            assert_eq!(UnifiedService::route_of(retired), None, "{retired}");
        }
        // `plan_` belongs to the plan family, which serves no `plan_work`.
        assert_eq!(UnifiedService::route_of("plan_work"), Some(Family::Plan));
    }

    /// Every prefix a family claims must route back to that family, and must be
    /// spellable by an operator — otherwise a whole namespace's tools fall
    /// through to the unknown-tool arm, or can be listed and never selected.
    #[test]
    fn every_claimed_prefix_routes_to_the_family_that_claims_it() {
        for family in Family::ALL {
            for prefix in family.prefixes() {
                let name = format!("{prefix}_anything");
                assert_eq!(
                    UnifiedService::route_of(&name),
                    Some(family),
                    "{name} should route to {family:?}"
                );
                assert_eq!(
                    Family::parse(prefix),
                    Some(family),
                    "an operator who writes '{prefix}' in {FAMILIES_ENV} should reach {family:?}"
                );
            }
        }
    }

    /// The two properties [`Family::prefixes`] must have for every caller that
    /// derives from it to be answering the same question.
    ///
    /// Empty would make [`Family::prefix`] answer with the empty string — which
    /// `String::starts_with` accepts from everything, so one empty arm would
    /// swallow every tool name in the server. Overlapping would make routing
    /// depend on the order of [`Family::ALL`], which is a declaration order
    /// nobody thinks of as load-bearing.
    #[test]
    fn every_family_claims_at_least_one_prefix_and_no_two_claim_the_same() {
        let mut claimed: Vec<(&str, Family)> = Vec::new();
        for family in Family::ALL {
            let prefixes = family.prefixes();
            assert!(
                !prefixes.is_empty(),
                "{family:?} claims no prefix, so Family::prefix answers with an empty \
                 string and every tool name in the server would route to it"
            );
            assert_eq!(
                family.prefix(),
                prefixes[0],
                "the naming prefix must be the head of the claim"
            );
            assert_eq!(&prefixes[1..], family.aliases());
            for prefix in prefixes {
                if let Some((_, other)) = claimed.iter().find(|(seen, _)| seen == prefix) {
                    panic!(
                        "prefix '{prefix}' is claimed by both {other:?} and {family:?}; \
                         which one a tool reaches would depend on Family::ALL's order"
                    );
                }
                claimed.push((prefix, family));
            }
        }
    }

    /// THE GAP THIS CLOSES, and it is the mirror image of
    /// `every_listed_tool_can_actually_be_routed`.
    ///
    /// That test walks the served list and demands each name route somewhere.
    /// This one walks the `initialize` block — the first thing a client reads,
    /// and the only description of the surface it gets before calling anything —
    /// and demands every namespace it advertises be one a `Family` actually
    /// claims. A block naming a prefix nothing claims tells a client to reach
    /// for tools that answer "unknown tool", which is a worse failure than not
    /// mentioning them: it is documentation pointing at a wall.
    ///
    /// Held against the LIVE `get_info`, not the constant, so an instructions
    /// block assembled differently one day is still checked.
    #[test]
    fn the_initialize_block_advertises_no_prefix_no_family_claims() {
        let info = service().get_info();
        let instructions = info
            .instructions
            .expect("the initialize block must carry instructions for a client to read");

        let advertised = advertised_prefixes(&instructions);
        // Non-vacuity: an extractor that found nothing would satisfy every
        // assertion below while checking no prose at all.
        assert!(
            advertised.len() >= Family::ALL.len(),
            "only {} namespaces were extracted from the block, fewer than the {} \
             families that exist — the gate is reading nothing: {advertised:?}",
            advertised.len(),
            Family::ALL.len()
        );

        let mut unclaimed: Vec<&str> = advertised
            .iter()
            .copied()
            .filter(|stem| Family::parse(stem).is_none())
            .collect();
        // A namespace named twice in the prose is one problem, not two.
        unclaimed.sort_unstable();
        unclaimed.dedup();
        assert!(
            unclaimed.is_empty(),
            "the initialize block advertises {unclaimed:?}, which no Family claims — \
             a client told about these namespaces gets 'unknown tool' when it uses \
             them. Claim the prefix in Family::prefixes or stop advertising it."
        );
    }

    /// The gate above, driven against a block this server has never served.
    ///
    /// Without this, `the_initialize_block_advertises_no_prefix_no_family_claims`
    /// could be passing because the check is inert rather than because the block
    /// is clean — the two are indistinguishable from a green run over one string.
    #[test]
    fn the_initialize_block_gate_names_a_prefix_no_family_claims() {
        let forged = "Five tool families share one server:\n\n  \
                      plan_* the plan\n  audit_* nothing claims this one\n";
        let unclaimed: Vec<&str> = advertised_prefixes(forged)
            .into_iter()
            .filter(|stem| Family::parse(stem).is_none())
            .collect();
        assert_eq!(
            unclaimed,
            vec!["audit"],
            "the gate must catch an advertised namespace no Family claims, and must \
             not accuse one that is claimed"
        );
    }

    /// The refusal must list the same families the table holds, aliases
    /// included — an operator who is told `plan` and then finds `tracker`
    /// works has been given a list that is not the list.
    #[test]
    fn the_unknown_family_refusal_lists_every_prefix_in_the_table() {
        let message = FamilySelectionError::Unknown {
            token: "tracer".into(),
        }
        .to_string();
        assert!(message.contains("tracer"), "the typo is named: {message}");
        for family in Family::ALL {
            for prefix in family.prefixes() {
                assert!(
                    message.contains(prefix),
                    "the refusal must name '{prefix}': {message}"
                );
            }
        }
    }

    #[test]
    fn route_of_rejects_unknown_prefixes() {
        assert_eq!(UnifiedService::route_of("audit_foo"), None);
        assert_eq!(UnifiedService::route_of("foo"), None);
        assert_eq!(UnifiedService::route_of(""), None);
    }

    /// The crate's own front page must name every family this server serves.
    ///
    /// # Why this test exists where it does
    ///
    /// The doc comment at the top of `lib.rs` is what docs.rs renders as the
    /// crate's front page, and what crates.io links to by default. It is the
    /// most-read prose this project publishes. It spent two releases claiming
    /// the server had two tool families, because `plan_*` and `request_*`
    /// arrived and nothing pointed back at that paragraph.
    ///
    /// [`Family::ALL`] already carried the guarantee — its own doc says callers
    /// that describe the surface should iterate it "so adding a fifth can't
    /// leave a doc behind". The front page simply was not one of those callers.
    /// It is now, and this test lives beside the table it derives from rather
    /// than beside the file it reads, so a new family and its gate are in the
    /// same field of view.
    ///
    /// # What this proves, and what it does not
    ///
    /// It proves each family's prefix is PRESENT in the front page. It does not
    /// prove the surrounding sentence is true, or even grammatical — a page
    /// reading "we do not support `request_*`" would satisfy it. Presence is
    /// what a text gate can honestly check; meaning needs a reader.
    #[test]
    fn the_crate_front_page_names_every_family_the_server_serves() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(concat!("src/", "lib.rs"));
        let source = crate::infra::source_gate::read_window(&path);

        // The window is the leading run of `//!` lines: the front page itself,
        // not the whole file. Without this, a prefix mentioned anywhere in
        // 30-odd module declarations would satisfy the assertion.
        let front_page: String = source
            .lines()
            .take_while(|l| l.trim_start().starts_with("//!") || l.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !front_page.trim().is_empty(),
            "the crate front page is empty — an empty docs.rs page covers nothing"
        );

        for family in Family::ALL {
            // Built rather than written so this test's own source cannot be
            // mistaken for the page it checks, and so the needle tracks the
            // real prefix instead of a second hand-typed copy of it.
            let needle = format!("{}{}", family.prefix(), "_");
            assert!(
                front_page.contains(&needle),
                "the crate front page never names '{needle}' — a family the server \
                 serves is missing from the page most readers see first:\n{front_page}"
            );
        }

        // Non-vacuity: the search can fail. Without this, a `contains` that
        // always returned true would pass every assertion above and the gate
        // would be decorative.
        let absent = concat!("nosuchfamily", "_");
        assert!(
            !front_page.contains(absent),
            "the front page names a family that does not exist, so this gate's \
             search proves nothing"
        );
    }
}
