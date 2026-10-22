//! Public transport composition. Each native bridge keeps its own owner admission
//! and reconnects only on the next request, never by replaying an in-flight effect.
use std::{path::PathBuf, sync::Arc};

use color_eyre::eyre;
use rmcp::{
    ErrorData, Peer, ServerHandler, ServiceExt,
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, CancelTaskParams, ContentBlock,
        GetTaskParams, GetTaskResult, Implementation, ListResourceTemplatesResult,
        ListResourcesResult, ListToolsResult, PaginatedRequestParams, ProtocolVersion,
        ReadResourceRequestParams, ReadResourceResponse, ServerConfig, UpdateTaskParams,
    },
    service::{RequestContext, RoleClient, RoleServer, RunningService},
};
use serde_json::Value;
use tokio::task::JoinHandle;

use super::{
    catalog::{Catalog, Owner},
    client::{self, ForwardClient},
    composition,
};

#[derive(Clone)]
pub(super) struct Service {
    pub workflow: Peer<RoleClient>,
    pub workspace: Peer<RoleClient>,
    pub catalog: Arc<tokio::sync::RwLock<Catalog>>,
    access: super::Access,
    forward: ForwardClient,
    info: ServerConfig,
}

struct NativeConnection {
    client: RunningService<RoleClient, ForwardClient>,
    bridge: JoinHandle<eyre::Result<()>>,
}

impl NativeConnection {
    async fn open(
        root: PathBuf,
        owner: Owner,
        forward: ForwardClient,
        flags: Vec<String>,
    ) -> eyre::Result<Self> {
        let (client_transport, server_transport) = tokio::io::duplex(256 * 1024);
        let (input, output) = tokio::io::split(server_transport);
        let bridge = tokio::spawn(async move {
            match owner {
                Owner::Workflow => crate::mcp_tools::run_transport(root, input, output).await,
                Owner::Workspace => {
                    let inputs = crate::config::inputs::ProjectInputs::from_env(&root);
                    crate::workspace_mcp::run_transport(&inputs, &flags, input, output).await
                }
            }
        });
        match forward.serve(client_transport).await {
            Ok(client) => Ok(Self { client, bridge }),
            Err(error) => match bridge.await {
                Ok(Err(cause)) => Err(cause),
                Err(cause) => Err(cause.into()),
                Ok(Ok(())) => Err(error.into()),
            },
        }
    }

    async fn close(self) -> eyre::Result<()> {
        self.client.cancel().await?;
        self.bridge.await?
    }
}

/// Serve the public endpoint after recognized upgrades. Full scaffold repair is explicit.
pub async fn run_stdio_server(
    root: PathBuf,
    flags: &[String],
    access: super::Access,
) -> eyre::Result<()> {
    let root = crate::mcp_target::project_root(&root)?;
    crate::scaffold::project_upgrade::ProjectUpgrade::apply(&root)?;
    let forward = ForwardClient::default();
    let workflow =
        NativeConnection::open(root.clone(), Owner::Workflow, forward.clone(), Vec::new()).await?;
    let workspace =
        match NativeConnection::open(root, Owner::Workspace, forward.clone(), flags.to_vec()).await
        {
            Ok(connection) => connection,
            Err(error) => {
                workflow.close().await?;
                return Err(error);
            }
        };
    let result = serve(&workflow, &workspace, forward, access).await;
    // Both owners must be settled even if either shutdown reports an error.
    let (workflow_closed, workspace_closed) = tokio::join!(workflow.close(), workspace.close());
    result?;
    workflow_closed?;
    workspace_closed
}

async fn serve(
    workflow: &NativeConnection,
    workspace: &NativeConnection,
    forward: ForwardClient,
    access: super::Access,
) -> eyre::Result<()> {
    let catalog = Catalog::new(
        workflow.client.list_all_tools().await?,
        workspace.client.list_all_tools().await?,
    )?;
    let native_info = workflow
        .client
        .peer_info()
        .ok_or_else(|| eyre::eyre!("Workflow connection did not initialize"))?;
    let mut info = ServerConfig::new(native_info.capabilities.clone());
    info.protocol_version = ProtocolVersion::V_2025_06_18;
    info.server_info = Implementation::new("magistr", env!("CARGO_PKG_VERSION"));
    info.instructions = Some("Use context to resume work and search/read/edit/write for source. Use discover to find advanced workflow and workspace operations and their schemas; execute composes them with magistr.call(name,args). Every operation uses native authorization. Failed compositions stop and report completed effects; inspect receipts before continuing.".into());
    let handler = Service {
        workflow: workflow.client.peer().clone(),
        workspace: workspace.client.peer().clone(),
        catalog: Arc::new(tokio::sync::RwLock::new(catalog)),
        access,
        forward,
        info,
    };
    let (input, output) = crate::mcp_bridge::stdio()?;
    let transport = crate::mcp_tools::protocol_cap::ProtocolCap::new(
        &handler,
        rmcp::transport::IntoTransport::<RoleServer, std::io::Error, _>::into_transport((
            input, output,
        )),
    );
    match handler.serve(transport).await {
        Ok(running) => {
            running.waiting().await?;
            Ok(())
        }
        Err(rmcp::service::ServerInitializeError::ConnectionClosed(_)) => Ok(()),
        Err(error) => Err(error.into()),
    }
}

impl Service {
    async fn refresh_catalog(&self) -> Result<(), ErrorData> {
        let workflow = self
            .workflow
            .list_all_tools()
            .await
            .map_err(client::error)?;
        let workspace = self
            .workspace
            .list_all_tools()
            .await
            .map_err(client::error)?;
        *self.catalog.write().await = Catalog::new(workflow, workspace)?;
        Ok(())
    }
    pub async fn invoke(
        &self,
        name: &str,
        args: Value,
        context: &RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        if !self.access.permits(name, &args) {
            return Err(ErrorData::invalid_params(
                format!(
                    "Operation {name} is not allowed in this {:?} connection",
                    self.access
                ),
                None,
            ));
        }
        let operation = self.catalog.read().await.operation(name)?.clone();
        let peer = match operation.owner {
            Owner::Workflow => &self.workflow,
            Owner::Workspace => &self.workspace,
        };
        client::call(
            peer,
            &operation.native_name,
            args,
            context.meta.clone(),
            context.ct.clone(),
        )
        .await
    }
}

pub(super) fn response(value: Value, failed: bool) -> CallToolResult {
    let mut result = CallToolResult::success(vec![ContentBlock::text(value.to_string())]);
    result.structured_content = Some(value);
    result.is_error = Some(failed);
    result
}

impl ServerHandler for Service {
    fn get_info(&self) -> ServerConfig {
        self.info.clone()
    }
    fn supported_protocol_versions(&self) -> std::borrow::Cow<'static, [ProtocolVersion]> {
        std::borrow::Cow::Borrowed(&[
            ProtocolVersion::V_2024_11_05,
            ProtocolVersion::V_2025_03_26,
            ProtocolVersion::V_2025_06_18,
        ])
    }
    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let _ = self.forward.outer.set(context.peer);
        self.refresh_catalog().await?;
        Ok(ListToolsResult::with_all_items(
            self.catalog.read().await.direct_tools(),
        ))
    }
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let _ = self.forward.outer.set(context.peer.clone());
        let args = Value::Object(request.arguments.unwrap_or_default());
        match request.name.as_ref() {
            "discover" => {
                self.refresh_catalog().await?;
                Ok(response(self.catalog.read().await.discover(&args)?, false).into())
            }
            "execute" => composition::execute(self, &args, &context)
                .await
                .map(Into::into),
            name => {
                if !self.catalog.read().await.operation(name)?.direct {
                    return Err(ErrorData::invalid_params(
                        "Use execute to call discovered operations.",
                        None,
                    ));
                }
                self.invoke(name, args, &context).await.map(Into::into)
            }
        }
    }
    async fn list_resources(
        &self,
        request: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        self.workflow
            .list_resources(request)
            .await
            .map_err(client::error)
    }
    async fn list_resource_templates(
        &self,
        request: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, ErrorData> {
        self.workflow
            .list_resource_templates(request)
            .await
            .map_err(client::error)
    }
    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, ErrorData> {
        self.workflow
            .read_resource(request)
            .await
            .map(Into::into)
            .map_err(client::error)
    }
    async fn get_task(
        &self,
        request: GetTaskParams,
        _: RequestContext<RoleServer>,
    ) -> Result<GetTaskResult, ErrorData> {
        let peer = if request.task_id == "index/build" {
            &self.workspace
        } else {
            &self.workflow
        };
        peer.get_task(request).await.map_err(client::error)
    }
    async fn on_custom_request(
        &self,
        request: rmcp::model::CustomRequest,
        _: RequestContext<RoleServer>,
    ) -> Result<rmcp::model::CustomResult, ErrorData> {
        if request.method != "tasks/list" {
            return Err(ErrorData::new(
                rmcp::model::ErrorCode::METHOD_NOT_FOUND,
                request.method,
                None,
            ));
        }
        match self
            .workspace
            .send_request(rmcp::model::ClientRequest::CustomRequest(request))
            .await
            .map_err(client::error)?
        {
            rmcp::model::ServerResult::CustomResult(result) => Ok(result),
            _ => Err(ErrorData::internal_error(
                "Unexpected workspace task listing",
                None,
            )),
        }
    }
    async fn update_task(
        &self,
        request: UpdateTaskParams,
        _: RequestContext<RoleServer>,
    ) -> Result<(), ErrorData> {
        if self.access != super::Access::Full {
            return Err(ErrorData::invalid_params(
                "This connection cannot change task state",
                None,
            ));
        }
        self.workflow
            .update_task(request)
            .await
            .map_err(client::error)
    }
    async fn cancel_task(
        &self,
        request: CancelTaskParams,
        _: RequestContext<RoleServer>,
    ) -> Result<(), ErrorData> {
        if self.access != super::Access::Full {
            return Err(ErrorData::invalid_params(
                "This connection cannot change task state",
                None,
            ));
        }
        self.workflow
            .cancel_task(request)
            .await
            .map_err(client::error)
    }
}

#[cfg(test)]
#[path = "service.tests.rs"]
mod tests;
