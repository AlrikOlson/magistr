use std::{borrow::Cow, sync::Arc};

use magistr_workspace::service::{IndexBuildTask, WorkspaceService};
use rmcp::{
    ErrorData, ServerHandler,
    model::{
        CallToolRequestParams, CallToolResponse, CustomRequest, CustomResult, ErrorCode,
        GetTaskParams, GetTaskResult, Implementation, ListToolsResult, PaginatedRequestParams,
        ProtocolVersion, ServerCapabilities, ServerInfo,
    },
    service::{RequestContext, RoleServer},
};
use serde_json::json;

use super::{dispatch, tasks, tool_surface};

/// The protocol eras this server will negotiate or advertise: everything
/// below 2026-07-28, as `magistr mcp-serve` caps them too, so every Claude
/// client accepts the result shapes.
const SERVED_PROTOCOL_VERSIONS: &[ProtocolVersion] = &[
    ProtocolVersion::V_2024_11_05,
    ProtocolVersion::V_2025_03_26,
    ProtocolVersion::V_2025_06_18,
];

/// The rmcp server over one admitted workspace. Cheap by design and
/// stateless by construction: every answer is a function of the workspace
/// and the request's own `_meta`.
#[derive(Clone)]
pub struct WorkspaceMcpService {
    service: Arc<WorkspaceService>,
}

impl WorkspaceMcpService {
    #[must_use]
    pub fn new(service: Arc<WorkspaceService>) -> Self {
        Self { service }
    }

    /// The composed workspace this server serves.
    #[must_use]
    pub fn workspace(&self) -> &Arc<WorkspaceService> {
        &self.service
    }
}

impl ServerHandler for WorkspaceMcpService {
    fn get_info(&self) -> ServerInfo {
        // SEP-2663: declaring tasks is what makes `tasks/get` reachable at
        // all — rmcp answers the tasks/* methods with -32601 unless the
        // server advertised the extension.
        let mut info = ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_tasks()
                .build(),
        )
        .with_instructions(tool_surface::INSTRUCTIONS)
        .with_server_info(Implementation::new("magistr", env!("CARGO_PKG_VERSION")));
        info.protocol_version = ProtocolVersion::V_2025_06_18;
        info
    }

    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        Cow::Borrowed(SERVED_PROTOCOL_VERSIONS)
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        // Core tools first, plugin tools appended under their derived
        // namespaces. A broken manifest drops its plugin from the list,
        // never the list itself.
        let mut tools = tool_surface::core_tools(self.service.offers_script());
        let (plugins, _notes) = self.service.plugin_tools();
        tools.extend(plugins.into_iter().map(tool_surface::plugin_tool));
        Ok(ListToolsResult::with_all_items(tools))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        dispatch::call(&self.service, request, &context)
            .await
            .map(Into::into)
    }

    async fn get_task(
        &self,
        request: GetTaskParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetTaskResult, ErrorData> {
        let task = self
            .service
            .index_task()
            .filter(|_| request.task_id == IndexBuildTask::ID)
            .ok_or_else(|| {
                ErrorData::invalid_params("Failed to retrieve task: Task not found", None)
            })?;
        Ok(GetTaskResult::new(tasks::detailed(&task)))
    }

    /// The retained `tasks/list`: the one derived task, or none.
    async fn on_custom_request(
        &self,
        request: CustomRequest,
        _context: RequestContext<RoleServer>,
    ) -> Result<CustomResult, ErrorData> {
        if request.method == "tasks/list" {
            let tasks: Vec<_> = self
                .service
                .index_task()
                .iter()
                .map(tasks::listing)
                .collect();
            return Ok(CustomResult::new(json!({ "tasks": tasks })));
        }
        Err(ErrorData::new(
            ErrorCode::METHOD_NOT_FOUND,
            request.method,
            None,
        ))
    }
}
