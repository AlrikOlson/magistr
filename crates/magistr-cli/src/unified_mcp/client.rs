//! Native clients forward authorization prompts and cancellations without retrying effects.
use std::sync::{Arc, OnceLock};
use std::time::Duration;

// Logging remains part of the negotiated MCP 2025-06-18 protocol.
#[allow(deprecated)]
use rmcp::{
    ClientHandler, ErrorData, Peer,
    model::{
        CallToolRequest, CallToolRequestParams, CallToolResult, CancelledNotificationParam,
        ClientCapabilities, ClientConfig, ClientRequest, ElicitRequestParams, ElicitResult,
        LoggingMessageNotificationParam, ProgressNotificationParam, ProtocolVersion,
        RequestMetaObject, ResourceUpdatedNotificationParam, ServerResult,
        TaskStatusNotificationParams,
    },
    service::{NotificationContext, PeerRequestOptions, RequestContext, RoleClient, RoleServer},
};
use serde_json::Value;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Default)]
pub(super) struct ForwardClient {
    pub outer: Arc<OnceLock<Peer<RoleServer>>>,
}

impl ClientHandler for ForwardClient {
    fn get_info(&self) -> ClientConfig {
        let mut info = ClientConfig::default();
        info.protocol_version = ProtocolVersion::V_2025_06_18;
        info.capabilities = ClientCapabilities::builder().enable_elicitation().build();
        info
    }

    async fn on_progress(
        &self,
        params: ProgressNotificationParam,
        _: NotificationContext<RoleClient>,
    ) {
        if let Some(peer) = self.outer.get() {
            let _ = peer.notify_progress(params).await;
        }
    }
    #[allow(deprecated)]
    async fn on_logging_message(
        &self,
        params: LoggingMessageNotificationParam,
        _: NotificationContext<RoleClient>,
    ) {
        if let Some(peer) = self.outer.get() {
            let _ = peer.notify_logging_message(params).await;
        }
    }
    async fn on_resource_updated(
        &self,
        params: ResourceUpdatedNotificationParam,
        _: NotificationContext<RoleClient>,
    ) {
        if let Some(peer) = self.outer.get() {
            let _ = peer.notify_resource_updated(params).await;
        }
    }
    async fn on_task_status(
        &self,
        params: TaskStatusNotificationParams,
        _: NotificationContext<RoleClient>,
    ) {
        if let Some(peer) = self.outer.get() {
            let _ = peer
                .send_notification(rmcp::model::ServerNotification::TaskStatusNotification(
                    rmcp::model::TaskStatusNotification::new(params),
                ))
                .await;
        }
    }
    async fn on_tool_list_changed(&self, _: NotificationContext<RoleClient>) {
        if let Some(peer) = self.outer.get() {
            let _ = peer.notify_tool_list_changed().await;
        }
    }
    async fn on_resource_list_changed(&self, _: NotificationContext<RoleClient>) {
        if let Some(peer) = self.outer.get() {
            let _ = peer.notify_resource_list_changed().await;
        }
    }

    async fn create_elicitation(
        &self,
        request: ElicitRequestParams,
        _: RequestContext<RoleClient>,
    ) -> Result<ElicitResult, ErrorData> {
        let peer = self
            .outer
            .get()
            .ok_or_else(|| ErrorData::internal_error("Client has not connected.", None))?;
        peer.create_elicitation(request).await.map_err(error)
    }
}

pub(super) fn error(error: impl std::fmt::Display) -> ErrorData {
    ErrorData::internal_error(error.to_string(), None)
}

pub(super) async fn call(
    peer: &Peer<RoleClient>,
    name: &str,
    args: Value,
    meta: RequestMetaObject,
    cancel: CancellationToken,
) -> Result<CallToolResult, ErrorData> {
    if cancel.is_cancelled() {
        return Err(ErrorData::internal_error(
            "Cancelled before dispatch; no operation was started.",
            None,
        ));
    }
    let params = CallToolRequestParams::new(name.to_owned()).with_arguments(
        args.as_object()
            .cloned()
            .ok_or_else(|| ErrorData::invalid_params("arguments must be an object", None))?,
    );
    let request = ClientRequest::CallToolRequest(CallToolRequest::new(params));
    let handle = peer
        .send_cancellable_request(request, PeerRequestOptions::no_options().with_meta(meta))
        .await
        .map_err(error)?;
    let request_id = handle.id.clone();
    let response = handle.await_response();
    tokio::pin!(response);
    let result = tokio::select! {
        result = &mut response => result.map_err(error)?,
        _ = cancel.cancelled() => {
            peer.notify_cancelled(CancelledNotificationParam::new(Some(request_id), Some("Client cancelled the operation".into()))).await.map_err(error)?;
            // Let the native operation settle its worker/process cleanup and report actual effects.
            match tokio::time::timeout(Duration::from_secs(10), &mut response).await {
                Ok(result) => result.map_err(error)?,
                Err(_) => return Err(ErrorData::internal_error("Cancellation sent; operation outcome is unknown. Read its current state before retrying.", None)),
            }
        }
    };
    match result {
        ServerResult::CallToolResult(result) => Ok(result),
        _ => Err(ErrorData::internal_error(
            "Unexpected native operation response",
            None,
        )),
    }
}

pub(super) fn value(result: &CallToolResult) -> Value {
    if let Some(value) = &result.structured_content {
        return value.clone();
    }
    if result.content.len() == 1
        && let Some(text) = result.content[0].as_text()
        && let Ok(value) = serde_json::from_str(&text.text)
    {
        return value;
    }
    serde_json::json!({"content":result.content,"isError":result.is_error})
}
