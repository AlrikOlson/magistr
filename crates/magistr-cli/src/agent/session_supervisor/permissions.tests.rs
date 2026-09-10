use super::*;
use crate::agent::session_supervisor::{SessionResidency, SessionSupervisorSnapshot};
use crate::permissions::NoninteractivePolicy;

fn state() -> Mutex<Inner> {
    Mutex::new(Inner {
        snapshot: SessionSupervisorSnapshot {
            active_turn: Some(1),
            queued_turns: 0,
            cancellation_requested: false,
            residency: SessionResidency::Resident,
            last_outcome: None,
            client_attached: true,
        },
        queue: Default::default(),
        next_turn: 2,
        completed_turns: 0,
        closing: false,
        legacy_cancel: false,
        control_busy: false,
        capabilities: None,
        messages: None,
    })
}

fn permissions(
    handling: NoninteractivePolicy,
) -> (Permissions, mpsc::UnboundedSender<AcpPermissionRequest>) {
    let (sender, receiver) = mpsc::unbounded_channel();
    let policy = crate::permissions::SessionPermissionPolicy::new(
        crate::permissions::PermissionProfile::Standard,
        vec![],
        vec![],
        vec![],
        false,
    )
    .audit_to(None);
    (
        Permissions::new(Some(receiver), Some((Box::new(policy), handling))),
        sender,
    )
}

fn request(
    options: &[(&str, magistr_acp::AcpPermissionOptionKind)],
) -> (
    AcpPermissionRequest,
    tokio::sync::oneshot::Receiver<AcpPermissionResponse>,
) {
    let (response_tx, response) = tokio::sync::oneshot::channel();
    (
        AcpPermissionRequest {
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
        response,
    )
}

#[tokio::test]
async fn legacy_policy_prefers_allow_once_and_never_selects_standing_or_reject_grants() {
    use magistr_acp::AcpPermissionOptionKind as Kind;
    for (options, expected) in [
        (
            vec![
                ("deny", Kind::RejectOnce),
                ("always", Kind::AllowAlways),
                ("once", Kind::AllowOnce),
            ],
            Some("once"),
        ),
        (
            vec![("deny", Kind::RejectOnce), ("always", Kind::AllowAlways)],
            None,
        ),
        (vec![("deny", Kind::RejectOnce)], None),
        (vec![], None),
    ] {
        let (mut permissions, _sender) = permissions(NoninteractivePolicy::LegacyAutomaticApproval);
        let (request, response) = request(&options);
        permissions.answer(Some(request), &state());
        match (response.await.unwrap(), expected) {
            (AcpPermissionResponse::Selected(id), Some(expected)) => assert_eq!(id, expected),
            (AcpPermissionResponse::Cancelled, None) => {}
            unexpected => panic!("unexpected response: {unexpected:?}"),
        }
    }
}

#[tokio::test]
async fn unattended_default_refuses_unanswered_edit_permission() {
    let (mut permissions, _sender) = permissions(Default::default());
    let (request, response) = request(&[("once", magistr_acp::AcpPermissionOptionKind::AllowOnce)]);
    permissions.answer(Some(request), &state());
    assert!(matches!(
        response.await,
        Ok(AcpPermissionResponse::Cancelled)
    ));
}

#[tokio::test]
async fn cancel_admission_refuses_queued_and_late_permissions_and_next_turn_starts_fresh() {
    let (mut permissions, sender) = permissions(NoninteractivePolicy::LegacyAutomaticApproval);
    let state = state();
    let options = [("once", magistr_acp::AcpPermissionOptionKind::AllowOnce)];
    let (queued, queued_response) = request(&options);
    sender.send(queued).unwrap();
    state.lock().unwrap().snapshot.cancellation_requested = true;
    let queued = permissions.recv().await;
    permissions.answer(queued, &state);
    let (late, late_response) = request(&options);
    permissions.answer(Some(late), &state);
    assert!(matches!(
        queued_response.await,
        Ok(AcpPermissionResponse::Cancelled)
    ));
    assert!(matches!(
        late_response.await,
        Ok(AcpPermissionResponse::Cancelled)
    ));

    let (stale, stale_response) = request(&options);
    sender.send(stale).unwrap();
    permissions.drain();
    {
        let mut inner = state.lock().unwrap();
        inner.snapshot.active_turn = Some(2);
        inner.snapshot.cancellation_requested = false;
    }
    let (fresh, fresh_response) = request(&options);
    permissions.answer(Some(fresh), &state);
    assert!(matches!(
        stale_response.await,
        Ok(AcpPermissionResponse::Cancelled)
    ));
    assert!(
        matches!(fresh_response.await, Ok(AcpPermissionResponse::Selected(id)) if id == "once")
    );
}

#[tokio::test]
async fn dropping_permission_owner_answers_queue_and_closes_admission() {
    let (permissions, sender) = permissions(NoninteractivePolicy::LegacyAutomaticApproval);
    let (request, response) = request(&[("once", magistr_acp::AcpPermissionOptionKind::AllowOnce)]);
    sender.send(request).unwrap();
    drop(permissions);
    assert!(matches!(
        response.await,
        Ok(AcpPermissionResponse::Cancelled)
    ));
    assert!(sender.is_closed());
}

#[tokio::test]
async fn stale_reply_does_not_disable_fresh_permission_handling() {
    let (mut permissions, _sender) = permissions(NoninteractivePolicy::LegacyAutomaticApproval);
    let options = [("once", magistr_acp::AcpPermissionOptionKind::AllowOnce)];
    let (stale, response) = request(&options);
    drop(response);
    permissions.answer(Some(stale), &state());
    let (fresh, response) = request(&options);
    permissions.answer(Some(fresh), &state());
    assert!(matches!(response.await, Ok(AcpPermissionResponse::Selected(id)) if id == "once"));
}
