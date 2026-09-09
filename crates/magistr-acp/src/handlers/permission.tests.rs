use agent_client_protocol::schema::v1::{
    PermissionOptionKind, RequestPermissionOutcome, ToolCallUpdate, ToolCallUpdateFields, ToolKind,
};

use super::*;
use crate::AcpPermissionOptionKind;

fn request(options: Vec<PermissionOption>) -> RequestPermissionRequest {
    RequestPermissionRequest::new(
        "sess",
        ToolCallUpdate::new(
            "tc-1",
            ToolCallUpdateFields::new()
                .title("Bash".to_string())
                .kind(ToolKind::Execute)
                .raw_input(serde_json::json!({"command": "ls"})),
        ),
        options,
    )
}

fn two_options() -> Vec<PermissionOption> {
    vec![
        PermissionOption::new("allow", "Allow", PermissionOptionKind::AllowOnce),
        PermissionOption::new("reject", "Reject", PermissionOptionKind::RejectOnce),
    ]
}

fn selected_id(response: &RequestPermissionResponse) -> Option<String> {
    match &response.outcome {
        RequestPermissionOutcome::Selected(s) => Some(s.option_id.to_string()),
        _ => None,
    }
}

#[tokio::test]
async fn forwards_the_request_and_returns_the_users_choice() {
    let (perm_tx, mut perm_rx) = mpsc::unbounded_channel::<AcpPermissionRequest>();
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();

    let answer = tokio::spawn(async move {
        let req = perm_rx.recv().await.expect("forwarded");
        assert_eq!(req.tool_title, "Bash");
        assert_eq!(req.tool_call_id, "tc-1");
        assert_eq!(req.tool_kind, Some(ToolKind::Execute));
        assert_eq!(req.tool_input["command"], "ls");
        assert_eq!(req.options.len(), 2);
        assert_eq!(req.options[1].kind, AcpPermissionOptionKind::RejectOnce);
        let _ = req
            .response_tx
            .send(AcpPermissionResponse::Selected("reject".into()));
    });

    let response = request_permission(request(two_options()), &perm_tx, &event_tx).await;
    answer.await.expect("answer task");
    assert_eq!(selected_id(&response).as_deref(), Some("reject"));

    // The UI is told about the request through the event stream too.
    let announced = event_rx.try_recv().expect("system event");
    assert!(matches!(
        announced,
        Ok(AcpEvent::System { subtype, .. }) if subtype == "permission_request"
    ));
}

#[tokio::test]
async fn a_cancelled_prompt_becomes_a_cancelled_outcome() {
    let (perm_tx, mut perm_rx) = mpsc::unbounded_channel::<AcpPermissionRequest>();
    let (event_tx, _event_rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        let req = perm_rx.recv().await.expect("forwarded");
        let _ = req.response_tx.send(AcpPermissionResponse::Cancelled);
    });
    let response = request_permission(request(two_options()), &perm_tx, &event_tx).await;
    assert!(matches!(
        response.outcome,
        RequestPermissionOutcome::Cancelled
    ));
}

/// Nobody to ask is a deny, never an allow on the user's behalf.
#[tokio::test]
async fn no_receiver_denies() {
    let (perm_tx, perm_rx) = mpsc::unbounded_channel::<AcpPermissionRequest>();
    drop(perm_rx);
    let (event_tx, _event_rx) = mpsc::unbounded_channel();
    let response = request_permission(request(two_options()), &perm_tx, &event_tx).await;
    assert_eq!(selected_id(&response).as_deref(), Some("reject"));
}

#[tokio::test]
async fn a_dropped_reply_denies() {
    let (perm_tx, mut perm_rx) = mpsc::unbounded_channel::<AcpPermissionRequest>();
    let (event_tx, _event_rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        let req = perm_rx.recv().await.expect("forwarded");
        drop(req.response_tx);
    });
    let response = request_permission(request(two_options()), &perm_tx, &event_tx).await;
    assert_eq!(selected_id(&response).as_deref(), Some("reject"));
}

/// An ask shown and never answered is denied when the timeout elapses,
/// and the stream says so with
/// the call's id, so the surface can take the card down. Paused time:
/// the 120 s pass the instant nothing else can run.
#[tokio::test(start_paused = true)]
async fn an_unanswered_request_is_denied_at_the_timeout_and_the_lapse_is_announced() {
    let (perm_tx, mut perm_rx) = mpsc::unbounded_channel::<AcpPermissionRequest>();
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    // The application shows the ask and holds the reply channel open
    // without ever writing it — a card nobody answers.
    let holder = tokio::spawn(async move {
        let req = perm_rx.recv().await.expect("forwarded");
        std::future::pending::<()>().await;
        drop(req);
    });
    let response = request_permission(request(two_options()), &perm_tx, &event_tx).await;
    holder.abort();
    assert_eq!(selected_id(&response).as_deref(), Some("reject"));
    let events: Vec<(String, serde_json::Value)> = std::iter::from_fn(|| event_rx.try_recv().ok())
        .filter_map(|e| match e {
            Ok(AcpEvent::System { subtype, data }) => Some((subtype, data)),
            _ => None,
        })
        .collect();
    assert_eq!(events.len(), 2, "{events:?}");
    assert_eq!(events[0].0, "permission_request");
    assert_eq!(events[1].0, PERMISSION_LAPSED);
    assert_eq!(events[1].1["tool_call_id"], "tc-1");
    assert_eq!(events[1].1["tool_title"], "Bash");
    assert_eq!(events[1].1["timeout_secs"], PERMISSION_TIMEOUT.as_secs());
}

/// The deny picks the reject the agent offered — once before always —
/// and cancels when it offered none.
#[test]
fn the_deny_fallback_is_a_reject_when_offered_else_a_cancel() {
    let response = deny_fallback(&[
        PermissionOption::new("allow", "Allow", PermissionOptionKind::AllowOnce),
        PermissionOption::new("never", "Never", PermissionOptionKind::RejectAlways),
        PermissionOption::new("reject", "Reject", PermissionOptionKind::RejectOnce),
    ]);
    assert_eq!(selected_id(&response).as_deref(), Some("reject"));
    let response = deny_fallback(&[
        PermissionOption::new("allow", "Allow", PermissionOptionKind::AllowOnce),
        PermissionOption::new("never", "Never", PermissionOptionKind::RejectAlways),
    ]);
    assert_eq!(selected_id(&response).as_deref(), Some("never"));
    let response = deny_fallback(&[PermissionOption::new(
        "allow",
        "Allow",
        PermissionOptionKind::AllowOnce,
    )]);
    assert!(
        matches!(response.outcome, RequestPermissionOutcome::Cancelled),
        "an allow-only ask is cancelled, never allowed"
    );
    let response = deny_fallback(&[]);
    assert!(matches!(
        response.outcome,
        RequestPermissionOutcome::Cancelled
    ));
}
