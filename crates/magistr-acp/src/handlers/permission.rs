//! `session/request_permission`: forward to the application, await its answer.

use std::time::Duration;

use agent_client_protocol::schema::v1::{
    PermissionOption, PermissionOptionKind, RequestPermissionOutcome, RequestPermissionRequest,
    RequestPermissionResponse, SelectedPermissionOutcome,
};
use tokio::sync::{mpsc, oneshot};

use crate::{AcpEvent, AcpPermissionOption, AcpPermissionRequest, AcpPermissionResponse, Result};

/// How long to wait for the application before answering on its behalf, so
/// an unrendered prompt (headless run, frontend bug) never wedges the agent.
pub(crate) const PERMISSION_TIMEOUT: Duration = Duration::from_secs(120);

/// The event the timeout announces itself with: `tool_call_id`,
/// `tool_title` and `timeout_secs`, so the surface that showed the ask
/// can take it down and name the denial.
pub(crate) const PERMISSION_LAPSED: &str = "permission_lapsed";

/// Forward a permission request to the application and return its answer.
///
/// A dropped receiver, dropped reply, or timeout denies the request using a
/// reject option, or cancellation when no reject option exists. Timeouts also
/// emit [`PERMISSION_LAPSED`] so the frontend can remove the pending prompt.
pub(crate) async fn request_permission(
    args: RequestPermissionRequest,
    perm_tx: &mpsc::UnboundedSender<AcpPermissionRequest>,
    event_tx: &mpsc::UnboundedSender<Result<AcpEvent>>,
) -> RequestPermissionResponse {
    let title = args
        .tool_call
        .fields
        .title
        .clone()
        .unwrap_or_else(|| "unknown".to_string());
    let tool_call_id = args.tool_call.tool_call_id.to_string();

    let _ = event_tx.send(Ok(AcpEvent::System {
        subtype: "permission_request".to_string(),
        data: serde_json::json!({
            "tool_call_id": &tool_call_id,
            "tool_title": &title,
        }),
    }));

    let (response_tx, response_rx) = oneshot::channel();
    let request = AcpPermissionRequest {
        tool_call_id: tool_call_id.clone(),
        tool_title: title.clone(),
        tool_input: args
            .tool_call
            .fields
            .raw_input
            .clone()
            .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new())),
        tool_kind: args.tool_call.fields.kind,
        options: args.options.iter().map(AcpPermissionOption::from).collect(),
        response_tx,
    };

    if perm_tx.send(request).is_err() {
        tracing::warn!(tool = %title, "no permission receiver, denying");
        return deny_fallback(&args.options);
    }

    match tokio::time::timeout(PERMISSION_TIMEOUT, response_rx).await {
        Ok(Ok(AcpPermissionResponse::Selected(option_id))) => RequestPermissionResponse::new(
            RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(option_id)),
        ),
        Ok(Ok(AcpPermissionResponse::Cancelled)) => {
            RequestPermissionResponse::new(RequestPermissionOutcome::Cancelled)
        }
        Ok(Err(_dropped)) => {
            tracing::warn!(tool = %title, "permission reply dropped, denying");
            deny_fallback(&args.options)
        }
        Err(_elapsed) => {
            tracing::warn!(
                tool = %title,
                timeout_secs = PERMISSION_TIMEOUT.as_secs(),
                "permission request timed out, denying"
            );
            let _ = event_tx.send(Ok(AcpEvent::System {
                subtype: PERMISSION_LAPSED.to_string(),
                data: serde_json::json!({
                    "tool_call_id": &tool_call_id,
                    "tool_title": &title,
                    "timeout_secs": PERMISSION_TIMEOUT.as_secs(),
                }),
            }));
            deny_fallback(&args.options)
        }
    }
}

/// The reject option — once before always — else a cancel: never an
/// allow, so nothing the user did not answer runs on their behalf.
pub(crate) fn deny_fallback(options: &[PermissionOption]) -> RequestPermissionResponse {
    let reject = options
        .iter()
        .find(|option| option.kind == PermissionOptionKind::RejectOnce)
        .or_else(|| {
            options
                .iter()
                .find(|option| option.kind == PermissionOptionKind::RejectAlways)
        });
    let outcome = match reject {
        Some(option) => RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(
            option.option_id.clone(),
        )),
        None => RequestPermissionOutcome::Cancelled,
    };
    RequestPermissionResponse::new(outcome)
}

#[cfg(test)]
#[path = "permission.tests.rs"]
mod tests;
