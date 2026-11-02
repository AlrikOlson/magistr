//! `RequestService` — the rmcp `ServerHandler` for the `request_*` family,
//! mirroring `PlanService`. A brand-new family with NO deprecated aliases.

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
use crate::plan::engine::PlanEngine;
use crate::request::application::{RequestError, RequestUseCases};
use crate::request::engine::RequestEngine;

const SERVER_INSTRUCTIONS: &str = r#"request-* captures and tracks stakeholder requests: questions, suggestions, concerns, bugs, and feedback raised about the project. A request is the local subset of the cloud wire envelope (docs/SIGNAL_CONTRACT.md); the local store is a CACHE of the cloud system-of-record.

When to call which tool:
  - Record a new stakeholder request       → request_capture
  - Reconstruct the inbox / get counts     → request_status
  - Read one request by id                  → request_get
  - Attach a cross-ref (decision:/task:/…)   → request_link
  - Churn on a request (enrich + advance)   → request_research
  - What's ready to raise (earned)         → request_ready
  - Raise one to the human                 → request_raise
  - Defer one (snooze) / drop it (dismiss)  → request_snooze / request_dismiss
  - Promote a validated request to a task  → request_promote

Lifecycle: new → triaged → researched → raised → promoted; any non-terminal
state may be dismissed. Illegal transitions are rejected. request_promote turns
a researched/raised request into a plan idea and writes
bidirectional cross-refs (task:X onto the request, request:X onto the task);
promoting twice is idempotent."#;

#[derive(Clone)]
pub struct RequestService {
    pub(super) application: RequestUseCases,
    pub(super) tool_router: ToolRouter<RequestService>,
}

impl RequestService {
    pub fn new(engine: RequestEngine) -> Self {
        Self::from_use_cases(RequestUseCases::new(Arc::new(Mutex::new(engine))))
    }

    /// Expose MCP over an existing admitted request application.
    pub fn from_use_cases(application: RequestUseCases) -> Self {
        Self {
            application,
            tool_router: Self::make_tool_router(),
        }
    }

    /// Wire the plan engine so `request_promote` can create tasks. Called by
    /// the host's composition root with the same handle the `PlanService` holds.
    pub fn with_plan(mut self, plan: Arc<Mutex<PlanEngine>>) -> Self {
        self.application = self.application.with_plan(plan);
        self
    }

    /// Hand out a clone of the shared request engine handle so the
    /// `PlanService` can compose a ready-request count into `plan_status`
    /// at the wire layer — without coupling the `PlanEngine` to
    /// the `RequestEngine`.
    pub fn engine(&self) -> Arc<Mutex<RequestEngine>> {
        self.application.engine()
    }

    /// The same typed policy used by MCP, for native workflow callers.
    pub fn use_cases(&self) -> RequestUseCases {
        self.application.clone()
    }

    /// The `request_*` tools view. No alias appending — a new family.
    pub fn list_tools_view(&self) -> Vec<rmcp::model::Tool> {
        let mut tools = self.tool_router.list_all();
        for tool in tools.iter_mut() {
            if let Some(schema) = crate::request::output_schemas::output_schema_for(&tool.name) {
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
        result: Result<T, RequestError>,
    ) -> Result<CallToolResult, ErrorData> {
        match result {
            Ok(value) => serde_json::to_value(value)
                .map(Self::ok_structured)
                .map_err(|error| ErrorData::internal_error(error.to_string(), None)),
            Err(RequestError::Poisoned) => Err(ErrorData::internal_error(
                RequestError::Poisoned.to_string(),
                None,
            )),
            Err(error) => {
                use crate::infra::tool_failure::ToolFailure;
                if error.effect() == "unknown"
                    && matches!(&error, RequestError::PartialPromotion { .. })
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

impl ServerHandler for RequestService {
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
