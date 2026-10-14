use std::sync::{Arc, Mutex};

use rmcp::{
    ErrorData, RoleServer, ServerHandler,
    handler::server::{router::tool::ToolRouter, tool::ToolCallContext},
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, Implementation, ListToolsResult,
        PaginatedRequestParams, ServerCapabilities, ServerInfo,
    },
    service::RequestContext,
};

use crate::mcp::cache::catalog;
use crate::ship::application::{ShipError, ShipUseCases};
use crate::ship::engine::ShipEngine;
use crate::ship::output_schemas;

/// Names a caller derives for the finalize verb instead of reading it: the
/// prefix is `ship_`, so `ship_ship` is the shape a model reaches for. Answered
/// with the real name (see `UnifiedService::replacement_for_retired`) rather
/// than a bare "unknown tool".
pub(crate) const MISDERIVED_FINALIZE_NAMES: [&str; 1] = ["ship_ship"];

const SERVER_INSTRUCTIONS: &str = r#"ship_* records structured execution traces for autonomous AI development.

When to call which tool:
  - Defining the goal and acceptance criteria  → ship_set_objective
  - Adding/reordering tasks in the plan        → ship_plan
  - Starting work on a task                    → ship_start
  - Logging an action within a task            → ship_record
  - Closing a task with what was produced      → ship_complete
  - Marking a task blocked                     → ship_block
  - Recording a quality gate result            → ship_check
  - Pausing for a human yes (webapp-answered)  → ship_gate_open, then poll ship_gate_wait
  - Shipping the completed objective           → ship_finalize
  - Getting current state after context loss   → ship_status
  - Exporting the full execution trace         → ship_export
  - Wiping everything (destructive)            → ship_reset

Always set an objective before planning tasks. Always plan tasks before
starting them. The `think_step` field on ship_record links an
execution action back to the think_* reasoning step that motivated it
(the field keeps its historical name). Every JSON-returning tool advertises an outputSchema
and emits structuredContent — prefer parsing that over the text content."#;

#[derive(Clone)]
pub struct ShipService {
    pub(super) engine: Arc<Mutex<ShipEngine>>,
    pub(super) use_cases: ShipUseCases,
    pub(super) tool_router: ToolRouter<ShipService>,
}

impl ShipService {
    pub fn new(engine: ShipEngine) -> Self {
        Self::from_use_cases(ShipUseCases::new(Arc::new(Mutex::new(engine))))
    }

    pub fn from_use_cases(use_cases: ShipUseCases) -> Self {
        Self {
            engine: use_cases.engine(),
            use_cases,
            tool_router: Self::make_tool_router(),
        }
    }

    pub fn use_cases(&self) -> ShipUseCases {
        self.use_cases.clone()
    }

    pub(super) fn typed_result<T: serde::Serialize>(
        result: Result<T, ShipError>,
    ) -> Result<CallToolResult, ErrorData> {
        match result {
            Ok(value) => Ok(Self::ok_structured(
                serde_json::to_value(value)
                    .map_err(|error| ErrorData::internal_error(error.to_string(), None))?,
            )),
            Err(error) if error.message() == "engine mutex poisoned" => Err(Self::poisoned()),
            Err(error) => Ok(match error.store_error() {
                Some(store) => crate::infra::tool_failure::ToolFailure::store(error.kind(), store),
                None => Self::err_structured(error.kind(), error.message()),
            }),
        }
    }

    /// Hand out a clone of the shared engine handle, as `RoadmapService::engine`
    /// does, so a host that mounts this family beside its own tools can record
    /// on the same trace the wire writes to — a quality gate the host ran
    /// itself (on its own pty, streamed live) recorded through
    /// `ShipEngine::record_check_full` with its real exit code, rather than a
    /// second engine over the same store that the guards here cannot see.
    pub fn engine(&self) -> Arc<Mutex<ShipEngine>> {
        Arc::clone(&self.engine)
    }

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

    pub(super) fn poisoned() -> ErrorData {
        ErrorData::internal_error("engine mutex poisoned", None)
    }

    pub(super) fn ok_structured(value: serde_json::Value) -> CallToolResult {
        crate::infra::tool_result::ok(value)
    }

    /// Logical failures are MCP errors with stable diagnostics and explicit recovery.
    pub(super) fn err_structured(kind: &str, message: impl Into<String>) -> CallToolResult {
        crate::infra::tool_failure::ToolFailure::rejected(kind, message, Vec::new())
    }
}

impl ServerHandler for ShipService {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
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
        let tools = self.list_tools_view();
        Ok(catalog(ListToolsResult::with_all_items(tools)))
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
