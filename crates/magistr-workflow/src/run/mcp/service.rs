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
use crate::run::application::{RunError, RunUseCases};
use crate::run::engine::RunEngine;
use crate::run::output_schemas;

const SERVER_INSTRUCTIONS: &str = r#"The run tools record each run: its goal, its steps, the progress made on each step, and its check results.

When to call which tool:
  - Opening a run with its goal and acceptance  → run_open
  - Adding, removing or reordering steps        → arrange_steps
  - Starting a step                             → start_step
  - Recording progress on the active step       → record_progress
  - Running and recording a check on a step     → check_step
  - Finishing a step with what it produced      → finish_step
  - Marking a step as waiting                   → run_block_step
  - Asking a person for approval                → approval_open, then approval_wait
  - Finishing the run                           → run_finish
  - Reading the current run after context loss  → run_status
  - Exporting the run history                   → run_export
  - Wiping every run (destructive)              → run_reset

Open a run before arranging steps, and arrange steps before starting them.
The `note` field on record_progress links progress back to the reasoning
step that motivated it. Every JSON-returning tool advertises an outputSchema and
emits structuredContent; prefer parsing that over the text content."#;

#[derive(Clone)]
pub struct RunService {
    pub(super) engine: Arc<Mutex<RunEngine>>,
    pub(super) use_cases: RunUseCases,
    pub(super) tool_router: ToolRouter<RunService>,
}

impl RunService {
    pub fn new(engine: RunEngine) -> Self {
        Self::from_use_cases(RunUseCases::new(Arc::new(Mutex::new(engine))))
    }

    pub fn from_use_cases(use_cases: RunUseCases) -> Self {
        Self {
            engine: use_cases.engine(),
            use_cases,
            tool_router: Self::make_tool_router(),
        }
    }

    pub fn use_cases(&self) -> RunUseCases {
        self.use_cases.clone()
    }

    pub(super) fn typed_result<T: serde::Serialize>(
        result: Result<T, RunError>,
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

    /// Hand out a clone of the shared engine handle, as `PlanService::engine`
    /// does, so a host that mounts this family beside its own tools can record
    /// on the same trace the wire writes to — a quality gate the host ran
    /// itself (on its own pty, streamed live) recorded through
    /// `RunEngine::record_check_full` with its real exit code, rather than a
    /// second engine over the same store that the guards here cannot see.
    pub fn engine(&self) -> Arc<Mutex<RunEngine>> {
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

impl ServerHandler for RunService {
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
