//! `RoadmapService` — the rmcp `ServerHandler` for the `roadmap_*` family,
//! mirroring `ShipService`. Unlike think/ship there are NO deprecated aliases:
//! `roadmap_*` is a brand-new family with no legacy names to carry.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub use crate::roadmap::application::resolve_workspace_root;

use rmcp::{
    ErrorData, RoleServer, ServerHandler,
    handler::server::{router::tool::ToolRouter, tool::ToolCallContext},
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, Implementation, ListToolsResult,
        PaginatedRequestParams, ServerCapabilities, ServerConfig,
    },
    service::RequestContext,
};

use crate::mcp::cache::catalog;
use crate::roadmap::application::{RoadmapError, RoadmapUseCases};
use crate::roadmap::engine::RoadmapEngine;
use crate::roadmap::output_schemas;
use crate::ship::ShipUseCases;

const SERVER_INSTRUCTIONS: &str = r#"roadmap-* drives the project roadmap: the long-horizon plan-of-plans that sits above ship_* objectives and links to think_* reasoning.

When to call which tool:
  - Add a phase/item to the plan          → roadmap_add_chunk
  - Move a chunk through its lifecycle     → roadmap_set_status
  - Edit a chunk's description/acceptance  → roadmap_update_chunk
  - Retire a chunk (kept for history)      → roadmap_obsolete_chunk
  - Suggest a reorder (human decides)      → roadmap_reprioritize
  - Pick the next ready chunk              → roadmap_next
  - Begin work on a chunk                  → roadmap_start_chunk
  - Close a chunk with proof-of-ship       → roadmap_complete_chunk
  - Attach a think:/task: cross-reference  → roadmap_link
  - Record a research refresh + its steps  → roadmap_record_refresh
  - Reconstruct the plan / get state       → roadmap_status
  - Write ROADMAP.md and get a receipt     → roadmap_export
  - See if/where the plan is mirrored      → tracker_status
  - Mirror this roadmap into a tracker     → tracker_setup

tracker_setup reaches a THIRD-PARTY system. Its `create_missing` defaults to
false and creating a destination cannot be undone from here, so only pass true
when the human has asked for it. A missing destination with create_missing false
writes nothing at all, which is the safe outcome, not a failure to work around.

roadmap_next returns the most urgent `pending` chunk (smallest `priority`
number — lower sorts earlier) whose deps are all `done`. roadmap_reprioritize NEVER reorders — it records a proposal for a human
to accept. Every JSON-returning tool advertises an outputSchema and emits
structuredContent."#;

#[derive(Clone)]
pub struct RoadmapService {
    pub(super) application: RoadmapUseCases,
    pub(super) tool_router: ToolRouter<RoadmapService>,
}

impl RoadmapService {
    /// Serve `engine` with `completion` — the ship family whose receipts
    /// govern every route to `done` — as its completion authority.
    pub fn new(engine: RoadmapEngine, completion: ShipUseCases) -> Self {
        Self::from_use_cases(RoadmapUseCases::new(
            Arc::new(Mutex::new(engine)),
            completion,
        ))
    }

    /// Expose MCP over the same application used by native workflow clients.
    pub fn from_use_cases(application: RoadmapUseCases) -> Self {
        Self {
            application,
            tool_router: Self::make_tool_router(),
        }
    }

    /// Configure the fixed workspace for file exports. Resolve it at server
    /// startup with [`resolve_workspace_root`]; unconfigured services support
    /// inline export only. Relative roots are captured against startup cwd.
    ///
    /// Returns an error if the path cannot be made absolute, including an
    /// empty path or a failure to resolve the current directory.
    pub fn with_workspace_root(mut self, root: impl Into<PathBuf>) -> std::io::Result<Self> {
        self.application = self.application.with_workspace_root(root)?;
        Ok(self)
    }

    /// Wire the signal engine so `roadmap_status` includes a pending-signal
    /// count. Called by the host's composition root with the same handle
    /// the `SignalService` holds.
    pub fn with_signal(mut self, signal: Arc<Mutex<crate::signal::engine::SignalEngine>>) -> Self {
        self.application = self.application.with_signal(signal);
        self
    }

    /// Hand out a clone of the shared engine handle so a sibling family (the
    /// `signal_*` family's promote path) can create roadmap chunks
    /// without reaching into the roadmap *engine* type directly. The coupling
    /// lives at the service/wire layer; signal depends on roadmap (acyclic).
    pub fn engine(&self) -> Arc<Mutex<RoadmapEngine>> {
        self.application.engine()
    }

    pub fn use_cases(&self) -> RoadmapUseCases {
        self.application.clone()
    }

    /// Build the `roadmap_*` tools view with output schemas patched on. No
    /// alias appending — this is a new family.
    pub fn list_tools_view(&self) -> Vec<rmcp::model::Tool> {
        let mut tools = self.tool_router.list_all();
        for tool in tools.iter_mut() {
            if let Some(schema) = output_schemas::output_schema_for(&tool.name) {
                tool.output_schema = Some(schema);
            }
            crate::mcp::schema_sanitize::sanitize_tool_schemas(tool);
        }
        tools
    }

    pub(super) fn ok_structured(value: serde_json::Value) -> CallToolResult {
        crate::infra::tool_result::ok(value)
    }

    /// Logical failures are MCP errors with stable diagnostics and explicit recovery.
    pub(super) fn err_structured(kind: &str, message: impl Into<String>) -> CallToolResult {
        crate::infra::tool_failure::ToolFailure::rejected(kind, message, Vec::new())
    }

    pub(super) fn present<T: serde::Serialize>(
        result: Result<T, RoadmapError>,
    ) -> Result<CallToolResult, ErrorData> {
        match result {
            Ok(value) => serde_json::to_value(value)
                .map(Self::ok_structured)
                .map_err(|error| ErrorData::internal_error(error.to_string(), None)),
            Err(RoadmapError::Poisoned) => Err(ErrorData::internal_error(
                "roadmap engine mutex poisoned",
                None,
            )),
            Err(error) => {
                use crate::infra::tool_failure::ToolFailure;
                if let Some(store) = error.store_error() {
                    Ok(ToolFailure::store(error.kind(), store))
                } else {
                    Ok(ToolFailure::rejected(
                        error.kind(),
                        error.to_string(),
                        Vec::new(),
                    ))
                }
            }
        }
    }
}

impl ServerHandler for RoadmapService {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
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

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        // Captured before the router consumes the request; `peer` is the only
        // route to the human, and it exists only inside a tools/call.
        let asked_about = Self::consent_question_for(request.name.as_ref());
        let peer = context.peer.clone();
        let tool = self
            .tool_router
            .list_all()
            .into_iter()
            .find(|tool| tool.name == request.name);
        if let Some(tool) = &tool
            && let Some(error) = crate::infra::tool_input::ToolInput::required(tool, &request)
        {
            return Ok(error.into());
        }
        let known = tool.is_some();
        let tcc = ToolCallContext::new(self, request, context);
        let result =
            crate::infra::tool_input::ToolInput::decoded(self.tool_router.call(tcc).await, known);

        // Ask AFTER the work, and only when the work actually landed. A
        // question on top of a failure is noise, and `tracker_setup` failing
        // means there is no mirroring to consent about yet.
        if asked_about && Self::landed(&result) {
            let (data_dir, _, _) = crate::tracker::host::tracker_config();
            // The outcome is deliberately dropped: by construction the ONLY
            // thing it can do is have already been remembered. Nothing here
            // touches `result`, so the caller's bytes are identical whether a
            // human was asked, declined, timed out, or was never there.
            let _ = crate::mcp::elicit::ask_and_remember_propose_consent(&peer, &data_dir).await;
        }
        result
    }
}

impl RoadmapService {
    /// Whether finishing this tool is the moment to ask about unattended
    /// proposals.
    ///
    /// Exactly one tool, and it is the onboarding one: a human who just
    /// connected their roadmap to a tracker is the only person for whom "may
    /// background sweeps propose?" is the next question rather than an
    /// interruption. Split out as a pure function so the *choice of moment* is
    /// assertable by exact name rather than by "some tool asks".
    #[must_use]
    pub fn consent_question_for(tool: &str) -> bool {
        tool == "tracker_setup"
    }

    /// Dependent consent runs only after a checked, readable success result.
    fn landed(result: &Result<CallToolResponse, ErrorData>) -> bool {
        let Ok(CallToolResponse::Complete(result)) = result else {
            return false;
        };
        crate::infra::checked_tool_result::CheckedToolResult::body(result).is_ok()
    }
}
