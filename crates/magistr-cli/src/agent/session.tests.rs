use super::*;
use crate::agent::message::{AssistantContent, ContentBlock};

fn assistant_msg(text: &str) -> AgentMessage {
    AgentMessage::Assistant {
        parent_tool_use_id: None,
        message: AssistantContent {
            model: "test-model".to_string(),
            content: vec![ContentBlock::Text {
                text: text.to_string(),
            }],
        },
        session_id: None,
    }
}

fn result_msg(session_id: &str) -> AgentMessage {
    AgentMessage::Result {
        subtype: "result".to_string(),
        duration_ms: 1000,
        duration_api_ms: 900,
        is_error: false,
        num_turns: 1,
        session_id: SessionId::new(session_id),
        total_cost_usd: Some(0.01),
        usage: None,
        result: None,
    }
}

#[tokio::test]
async fn session_from_channel_receives_messages() {
    let (mut session, tx) = AgentSession::from_channel();

    tx.send(Ok(assistant_msg("Hello from test"))).unwrap();
    tx.send(Ok(result_msg("test-sess-1"))).unwrap();

    let msg = session.next_message().await.unwrap().unwrap();
    assert!(matches!(msg, AgentMessage::Assistant { .. }));

    let msg = session.next_message().await.unwrap().unwrap();
    assert!(matches!(msg, AgentMessage::Result { .. }));
    assert_eq!(session.session_id().unwrap().as_str(), "test-sess-1");
}

#[tokio::test]
async fn session_send_followup_captured() {
    let (session, _tx, follow_ups) = AgentSession::from_channel_with_followups();

    assert!(session.send_followup("follow up text").is_ok());
    let captured = follow_ups.lock().unwrap();
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0], "follow up text");
}

#[tokio::test]
async fn session_abort_is_idempotent() {
    let (mut session, _tx) = AgentSession::from_channel();

    // Multiple aborts should not panic
    session.abort();
    session.abort();
}

#[tokio::test]
async fn session_tracks_session_id_across_turns() {
    let (mut session, tx) = AgentSession::from_channel();

    assert!(session.session_id().is_none());

    // Turn 1
    tx.send(Ok(assistant_msg("turn 1"))).unwrap();
    tx.send(Ok(result_msg("sess-turn-1"))).unwrap();
    let _ = session.next_message().await;
    let _ = session.next_message().await;
    assert_eq!(session.session_id().unwrap().as_str(), "sess-turn-1");

    // Turn 2 — session_id updates
    tx.send(Ok(assistant_msg("turn 2"))).unwrap();
    tx.send(Ok(result_msg("sess-turn-2"))).unwrap();
    let _ = session.next_message().await;
    let _ = session.next_message().await;
    assert_eq!(session.session_id().unwrap().as_str(), "sess-turn-2");
}

#[tokio::test]
async fn session_returns_none_when_stream_ends() {
    let (mut session, tx) = AgentSession::from_channel();

    // Drop the sender to close the stream
    drop(tx);

    let result = session.next_message().await;
    assert!(result.is_none());
}

#[tokio::test]
async fn session_cancel_is_noop_for_channel() {
    let (mut session, _tx) = AgentSession::from_channel();
    // cancel should not panic for channel backend
    session.cancel();
}

#[test]
fn session_state_returns_none_for_channel() {
    let (session, _tx) = AgentSession::from_channel();
    assert!(session.session_state().is_none());
}

#[test]
fn acp_session_state_eq() {
    assert_eq!(AcpSessionState::Created, AcpSessionState::Created);
    assert_ne!(AcpSessionState::Created, AcpSessionState::InProgress);
    assert_ne!(AcpSessionState::Cancelled, AcpSessionState::Failed);
}

fn test_policy() -> Box<dyn magistr_cockpit::agent::SessionPolicy> {
    Box::new(
        crate::permissions::SessionPermissionPolicy::new(
            crate::permissions::PermissionProfile::Standard,
            vec![],
            vec![],
            vec![],
            false,
        )
        .audit_to(None),
    )
}

fn permission_request(
    options: &[(&str, magistr_acp::AcpPermissionOptionKind)],
) -> (
    magistr_acp::AcpPermissionRequest,
    tokio::sync::oneshot::Receiver<magistr_acp::AcpPermissionResponse>,
) {
    let (response_tx, response_rx) = tokio::sync::oneshot::channel();
    (
        magistr_acp::AcpPermissionRequest {
            tool_call_id: "headless-call".into(),
            tool_title: "Edit".into(),
            tool_input: serde_json::json!({"file_path": ".magistr/project.json"}),
            tool_kind: Some(agent_client_protocol::schema::v1::ToolKind::Edit),
            options: options
                .iter()
                .map(|(id, kind)| magistr_acp::AcpPermissionOption {
                    option_id: (*id).into(),
                    kind: *kind,
                })
                .collect(),
            response_tx,
        },
        response_rx,
    )
}

async fn permission_response(
    options: &[(&str, magistr_acp::AcpPermissionOptionKind)],
    handling: crate::permissions::NoninteractivePolicy,
) -> magistr_acp::AcpPermissionResponse {
    let (mut session, _messages) = AgentSession::from_channel();
    let (requests, receiver) = mpsc::unbounded_channel();
    session.acp_permission_rx = Some(receiver);
    assert!(session.spawn_permission_worker(test_policy(), handling));
    assert!(session.take_acp_permission_rx().is_none());
    assert!(!session.spawn_permission_worker(test_policy(), handling));
    let task = session.permission_worker.take().unwrap();
    let (request, response_rx) = permission_request(options);
    assert!(requests.send(request).is_ok());
    drop(requests);
    tokio::time::timeout(std::time::Duration::from_secs(2), task)
        .await
        .expect("permission worker ends when its request stream closes")
        .expect("permission worker succeeds");
    response_rx.await.expect("the worker answers the request")
}

#[tokio::test]
async fn legacy_permissions_prefer_allow_once_under_the_project_policy() {
    use magistr_acp::{AcpPermissionOptionKind as Kind, AcpPermissionResponse};
    let response = permission_response(
        &[
            ("deny", Kind::RejectOnce),
            ("always", Kind::AllowAlways),
            ("once", Kind::AllowOnce),
        ],
        crate::permissions::NoninteractivePolicy::LegacyAutomaticApproval,
    )
    .await;
    assert!(matches!(response, AcpPermissionResponse::Selected(id) if id == "once"));
}

#[tokio::test]
async fn legacy_permissions_cancel_instead_of_selecting_rejection_or_standing_grant() {
    use magistr_acp::{AcpPermissionOptionKind as Kind, AcpPermissionResponse};
    let response = permission_response(
        &[("deny", Kind::RejectOnce), ("always", Kind::AllowAlways)],
        crate::permissions::NoninteractivePolicy::LegacyAutomaticApproval,
    )
    .await;
    assert!(matches!(response, AcpPermissionResponse::Cancelled));
}

#[tokio::test]
async fn noninteractive_permissions_cancel_when_no_allow_option_is_offered() {
    use magistr_acp::{AcpPermissionOptionKind as Kind, AcpPermissionResponse};
    for options in [&[][..], &[("deny", Kind::RejectOnce)][..]] {
        let response = permission_response(
            options,
            crate::permissions::NoninteractivePolicy::LegacyAutomaticApproval,
        )
        .await;
        assert!(matches!(response, AcpPermissionResponse::Cancelled));
    }
}

#[test]
fn a_taken_permission_receiver_cannot_be_claimed_by_the_headless_worker() {
    let (mut session, _messages) = AgentSession::from_channel();
    let (requests, receiver) = mpsc::unbounded_channel();
    session.acp_permission_rx = Some(receiver);
    let receiver = session.take_acp_permission_rx().unwrap();
    assert!(session.take_acp_permission_rx().is_none());
    assert!(!session.spawn_permission_worker(test_policy(), Default::default()));
    assert!(!requests.is_closed(), "the caller still owns the receiver");
    drop(receiver);
    assert!(requests.is_closed());
}

#[tokio::test]
async fn noninteractive_default_refuses_unanswered_edits() {
    use magistr_acp::{AcpPermissionOptionKind as Kind, AcpPermissionResponse};
    let response = permission_response(&[("once", Kind::AllowOnce)], Default::default()).await;
    assert!(matches!(response, AcpPermissionResponse::Cancelled));
}

#[tokio::test]
async fn cancelling_a_session_answers_queued_requests_with_cancelled() {
    use magistr_acp::{AcpPermissionOptionKind as Kind, AcpPermissionResponse};
    let (mut session, _messages) = AgentSession::from_channel();
    let (requests, receiver) = mpsc::unbounded_channel();
    session.acp_permission_rx = Some(receiver);
    session.spawn_permission_worker(
        test_policy(),
        crate::permissions::NoninteractivePolicy::LegacyAutomaticApproval,
    );
    let (request, response) = permission_request(&[("once", Kind::AllowOnce)]);
    requests.send(request).unwrap();
    session.cancel();
    assert!(matches!(
        response.await,
        Ok(AcpPermissionResponse::Cancelled)
    ));
    assert!(requests.is_closed());
}

#[tokio::test]
async fn dropping_a_session_stops_its_permission_worker() {
    use magistr_acp::{AcpPermissionOptionKind as Kind, AcpPermissionResponse};
    let (mut session, _messages) = AgentSession::from_channel();
    let (requests, receiver) = mpsc::unbounded_channel();
    session.acp_permission_rx = Some(receiver);
    session.spawn_permission_worker(
        test_policy(),
        crate::permissions::NoninteractivePolicy::LegacyAutomaticApproval,
    );
    let (request, response) = permission_request(&[("once", Kind::AllowOnce)]);
    requests.send(request).unwrap();
    drop(session);
    assert!(matches!(
        response.await,
        Ok(AcpPermissionResponse::Cancelled)
    ));
    assert!(requests.is_closed());
}

#[tokio::test]
async fn a_dropped_reply_does_not_stop_the_permission_worker() {
    use magistr_acp::{AcpPermissionOptionKind as Kind, AcpPermissionResponse};
    let (mut session, _messages) = AgentSession::from_channel();
    let (requests, receiver) = mpsc::unbounded_channel();
    session.acp_permission_rx = Some(receiver);
    session.spawn_permission_worker(test_policy(), Default::default());
    let (request, response) = permission_request(&[("once", Kind::AllowOnce)]);
    drop(response);
    requests.send(request).unwrap();
    let (request, response) = permission_request(&[("once", Kind::AllowOnce)]);
    requests.send(request).unwrap();
    assert!(matches!(
        response.await,
        Ok(AcpPermissionResponse::Cancelled)
    ));
}

#[test]
fn cancelled_and_aborted_sessions_refuse_followup_input() {
    let (mut cancelled, _messages, captured) = AgentSession::from_channel_with_followups();
    cancelled.cancel();
    assert!(
        cancelled
            .send_followup("Must not start another turn")
            .is_err()
    );
    assert!(captured.lock().unwrap().is_empty());
    let (mut aborted, _messages) = AgentSession::from_channel();
    aborted.abort();
    assert!(
        aborted
            .send_followup("Must not start another turn")
            .is_err()
    );
}
