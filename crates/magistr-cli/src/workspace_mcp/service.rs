use std::{borrow::Cow, sync::Arc};

use magistr_workspace::{
    contracts::WorkspaceError,
    service::{IndexBuildTask, WorkspaceService, WorkspaceSession},
};
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

use super::{dispatch, session_lifetime::SessionLifetime, tasks, tool_surface};

/// The protocol eras this server will negotiate or advertise: everything
/// below 2026-07-28, as `magistr mcp-serve` caps them too, so every Claude
/// client accepts the result shapes.
const SERVED_PROTOCOL_VERSIONS: &[ProtocolVersion] = &[
    ProtocolVersion::V_2024_11_05,
    ProtocolVersion::V_2025_03_26,
    ProtocolVersion::V_2025_06_18,
];

/// The rmcp server over one admitted workspace. Cheap by design and
/// carries one fallback identity per connection. Explicit request `_meta`
/// identities are admitted against the same workspace owner. Construct a new
/// handler for each connection; clones belong to the same connection identity.
#[derive(Clone)]
pub struct WorkspaceMcpService {
    service: Arc<WorkspaceService>,
    session: WorkspaceSession,
    // Last field: completion signals only after this handler's service reference drops.
    lifetime: Arc<SessionLifetime>,
}

impl WorkspaceMcpService {
    pub fn new(service: Arc<WorkspaceService>) -> Result<Self, WorkspaceError> {
        let session = service.new_session()?;
        Ok(Self {
            service,
            session,
            lifetime: Arc::default(),
        })
    }

    /// Completes after the final handler clone and every rmcp request task
    /// has released this connection, beyond the service loop's bounded drain.
    pub(super) fn completion(&self) -> tokio_util::sync::CancellationToken {
        self.lifetime.completion()
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
        // Run must finish its existing cancellation path to kill and reap its
        // owned process group; search likewise settles its worker cancellation.
        let joins_cleanup = matches!(request.name.as_ref(), "run" | "search");
        let response = dispatch::call(&self.service, &self.session, request, &context);
        if joins_cleanup {
            return response.await.map(Into::into);
        }
        tokio::select! {
            biased;
            _ = context.ct.cancelled() => Err(ErrorData::internal_error("workspace request cancelled before completion", None)),
            response = response => response.map(Into::into),
        }
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
