//! `SignalService` — the rmcp `ServerHandler` for the `signal_*` family,
//! mirroring `RoadmapService`. A brand-new family with NO deprecated aliases.

use std::sync::{Arc, Mutex};

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
use crate::roadmap::engine::RoadmapEngine;
use crate::signal::application::{SignalError, SignalUseCases};
use crate::signal::engine::SignalEngine;

const SERVER_INSTRUCTIONS: &str = r#"signal-* captures and tracks stakeholder signals: questions, ideas, concerns, bugs, and feedback raised about the project. A signal is the local subset of the cloud wire envelope (docs/SIGNAL_CONTRACT.md); the local store is a CACHE of the cloud system-of-record.

When to call which tool:
  - Record a new stakeholder signal       → signal_capture
  - Reconstruct the inbox / get counts     → signal_status
  - Read one signal by id                  → signal_get
  - Attach a cross-ref (think:/chunk:/…)   → signal_link
  - Churn on a signal (enrich + advance)   → signal_research
  - What's ready to raise (earned)         → signal_pending
  - Raise one to the human                 → signal_surface
  - Defer one (snooze) / drop it (ignore)  → signal_snooze / signal_ignore
  - Promote a validated signal to a chunk  → signal_promote

Lifecycle: new → triaged → researched → surfaced → promoted; any non-terminal
state may be dismissed. Illegal transitions are rejected. signal_promote turns
a researched/surfaced signal into a backlog roadmap chunk and writes
bidirectional cross-refs (chunk:X onto the signal, signal:X onto the chunk);
promoting twice is idempotent."#;

#[derive(Clone)]
pub struct SignalService {
    pub(super) application: SignalUseCases,
    pub(super) tool_router: ToolRouter<SignalService>,
}

impl SignalService {
    pub fn new(engine: SignalEngine) -> Self {
        Self::from_use_cases(SignalUseCases::new(Arc::new(Mutex::new(engine))))
    }

    /// Expose MCP over an existing admitted signal application.
    pub fn from_use_cases(application: SignalUseCases) -> Self {
        Self {
            application,
            tool_router: Self::make_tool_router(),
        }
    }

    /// Wire the roadmap engine so `signal_promote` can create chunks. Called by
    /// the host's composition root with the same handle the `RoadmapService` holds.
    pub fn with_roadmap(mut self, roadmap: Arc<Mutex<RoadmapEngine>>) -> Self {
        self.application = self.application.with_roadmap(roadmap);
        self
    }

    /// Hand out a clone of the shared signal engine handle so the
    /// `RoadmapService` can compose a pending-signal count into `roadmap_status`
    /// at the wire layer — without coupling the `RoadmapEngine` to
    /// the `SignalEngine`.
    pub fn engine(&self) -> Arc<Mutex<SignalEngine>> {
        self.application.engine()
    }

    /// The same typed policy used by MCP, for native workflow callers.
    pub fn use_cases(&self) -> SignalUseCases {
        self.application.clone()
    }

    /// The `signal_*` tools view. No alias appending — a new family.
    pub fn list_tools_view(&self) -> Vec<rmcp::model::Tool> {
        let mut tools = self.tool_router.list_all();
        for tool in tools.iter_mut() {
            if let Some(schema) = crate::signal::output_schemas::output_schema_for(&tool.name) {
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
        result: Result<T, SignalError>,
    ) -> Result<CallToolResult, ErrorData> {
        match result {
            Ok(value) => serde_json::to_value(value)
                .map(Self::ok_structured)
                .map_err(|error| ErrorData::internal_error(error.to_string(), None)),
            Err(SignalError::Poisoned) => Err(ErrorData::internal_error(
                SignalError::Poisoned.to_string(),
                None,
            )),
            Err(error) => {
                use crate::infra::tool_failure::ToolFailure;
                if error.effect() == "unknown"
                    && matches!(&error, SignalError::PartialPromotion { .. })
                {
                    Ok(ToolFailure::uncertain(error.kind(), error.to_string()))
                } else if let Some(store) = error.store_error() {
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

impl ServerHandler for SignalService {
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
        crate::infra::tool_input::ToolInput::decoded(self.tool_router.call(tcc).await, known)
    }
}
