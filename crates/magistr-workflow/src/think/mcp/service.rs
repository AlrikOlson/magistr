//! `ThinkService` — the MCP-facing handler.
//!
//! This module owns:
//!
//! * the service struct that wraps [`ReasoningServer`] in a `Mutex` so
//!   the rmcp routing layer (which calls handlers via `&self`) can
//!   mutate shared state,
//! * non-handler helpers (`new`, `engine`, `poisoned`, `list_tools_view`,
//!   `structured_err`), and
//! * the **manual** `ServerHandler` impl — manual rather than
//!   `#[tool_handler]`-generated because we override `list_tools` to
//!   patch each tool entry's `output_schema` with a
//!   `schemars::schema_for!(T)` blob (see [`output_schema_for`](crate::think::output_schemas::output_schema_for)).
//!
//! The 11 `#[tool]` handler methods live in [`super::handlers`], in a
//! sibling `impl` block annotated with `#[tool_router]`. Multiple impl
//! blocks for the same type are fine in Rust; the generated
//! `tool_router()` constructor is accessible from this file because
//! both impls share the `ThinkService` namespace.

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
use crate::think::engine::core::ReasoningServer;
use crate::think::output_schemas;

use super::instructions::SERVER_INSTRUCTIONS;

#[cfg(test)]
#[path = "service.tests.rs"]
mod tests;

/// The MCP-facing handler. Wraps the engine in a Mutex so the rmcp routing
/// layer (which calls handlers via &self) can mutate the shared state.
#[derive(Clone)]
pub struct ThinkService {
    pub(super) application: crate::think::application::ThinkUseCases,
    pub(super) tool_router: ToolRouter<ThinkService>,
}

impl ThinkService {
    pub fn new(engine: ReasoningServer) -> Self {
        Self::from_use_cases(crate::think::application::ThinkUseCases::new(Arc::new(
            Mutex::new(engine),
        )))
    }

    pub fn from_use_cases(application: crate::think::application::ThinkUseCases) -> Self {
        Self {
            application,
            tool_router: Self::make_tool_router(),
        }
    }

    /// Native application capability over exactly the engine served by MCP.
    pub fn use_cases(&self) -> crate::think::application::ThinkUseCases {
        self.application.clone()
    }

    /// Test helper — hands out the inner engine handle.
    pub fn engine(&self) -> Arc<Mutex<ReasoningServer>> {
        self.application.engine()
    }

    pub(super) fn native_result<T: serde::Serialize>(
        result: Result<T, String>,
        kind: &str,
    ) -> Result<CallToolResult, ErrorData> {
        match result {
            Ok(value) => serde_json::to_value(value)
                .map(CallToolResult::structured)
                .map_err(|error| ErrorData::internal_error(error.to_string(), None)),
            Err(error) if error == "engine mutex poisoned" => Err(Self::poisoned()),
            Err(error) => Ok(Self::structured_err(kind, error)),
        }
    }

    pub(super) fn mutation_result<T: serde::Serialize>(
        result: Result<T, crate::think::domain::StepError>,
        kind: &str,
    ) -> Result<CallToolResult, ErrorData> {
        match result {
            Ok(value) => serde_json::to_value(value)
                .map(CallToolResult::structured)
                .map_err(|error| ErrorData::internal_error(error.to_string(), None)),
            Err(error) => Self::step_failure(error, kind),
        }
    }

    pub(super) fn step_failure(
        error: crate::think::domain::StepError,
        validation_kind: &str,
    ) -> Result<CallToolResult, ErrorData> {
        if error.error == "engine mutex poisoned" {
            return Err(Self::poisoned());
        }
        let mut result = match &error.store {
            Some(store) => crate::infra::tool_failure::ToolFailure::store(error.kind(), store),
            None => crate::infra::tool_failure::ToolFailure::rejected(
                validation_kind,
                &error.error,
                vec![],
            ),
        };
        if let Some(body) = result
            .structured_content
            .as_mut()
            .and_then(serde_json::Value::as_object_mut)
        {
            body.insert("error".into(), serde_json::json!(error.error));
            body.insert("status".into(), serde_json::json!(error.status));
            body.insert("hint".into(), serde_json::json!(error.hint));
            if let Some(id) = error.record_id {
                body.insert("record_id".into(), serde_json::json!(id));
            }
        }
        result.content = vec![rmcp::model::ContentBlock::text(
            result
                .structured_content
                .as_ref()
                .expect("structured refusal")
                .to_string(),
        )];
        Ok(result)
    }

    pub(super) fn poisoned() -> ErrorData {
        ErrorData::internal_error("engine mutex poisoned", None)
    }

    /// Test helper — returns the `tools/list` view exactly as 2026 clients
    /// see it, including patched `output_schema` and `annotations`.
    pub fn list_tools_view(&self) -> Vec<rmcp::model::Tool> {
        let mut tools = self.tool_router.list_all();
        for tool in tools.iter_mut() {
            if tool.name == "think_record_step" {
                super::record_input::RecordInput::publish(tool);
            }
            if let Some(schema) = output_schemas::output_schema_for(&tool.name) {
                tool.output_schema = Some(schema);
            }
            crate::mcp::schema_sanitize::sanitize_tool_schemas(tool);
        }
        tools
    }

    /// Logical failures are MCP errors with stable diagnostics and explicit recovery.
    pub(super) fn structured_err(error_kind: &str, message: impl Into<String>) -> CallToolResult {
        crate::infra::tool_result::soft_error(error_kind, message)
    }
}

impl ServerHandler for ThinkService {
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
        if request.name == "think_record_step" {
            let arguments = request.arguments.clone().unwrap_or_default();
            if let Some(error) = super::record_input::RecordInput::validate(&arguments) {
                return Ok(error.into());
            }
        }
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
