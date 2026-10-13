use super::*;
use crate::agent::session_supervisor::{SessionResidency, SessionSupervisorSnapshot};
use crate::permissions::NoninteractivePolicy;

fn state() -> Mutex<Inner> {
    Mutex::new(Inner {
        history: None,
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
        max_turns: None,
        closing: false,
        draining: false,
        legacy_cancel: false,
        control_busy: false,
        goal_turn: None,
        capabilities: None,
        messages: None,
        cockpit_events: None,
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

struct RecordingPolicy {
    decision: magistr_application::sessions::PolicyDecision,
    answers: std::sync::Arc<Mutex<Vec<magistr_application::sessions::PermissionKind>>>,
}

impl magistr_application::sessions::SessionPolicy for RecordingPolicy {
    fn decide(
        &mut self,
        _request: &magistr_application::sessions::SessionPolicyRequest,
    ) -> magistr_application::sessions::PolicyDecision {
        self.decision
    }

    fn answered(
        &mut self,
        _request: &magistr_application::sessions::SessionPolicyRequest,
        kind: magistr_application::sessions::PermissionKind,
    ) {
        self.answers.lock().unwrap().push(kind);
    }
}

fn interactive() -> (
    Permissions,
    crate::agent::observation::ObservationReceiver<magistr_application::sessions::AgentEvent>,
    std::sync::Arc<Mutex<Vec<magistr_application::sessions::PermissionKind>>>,
) {
    let (events, receiver) =
        crate::agent::observation::channel(crate::agent::observation::ObservationCapacity::DEFAULT);
    let answers = std::sync::Arc::new(Mutex::new(Vec::new()));
    let policy = RecordingPolicy {
        decision: magistr_application::sessions::PolicyDecision::Ask(
            magistr_engine::policy::RiskTier::Medium,
        ),
        answers: std::sync::Arc::clone(&answers),
    };
    (
        Permissions::interactive(None, Box::new(policy), events),
        receiver,
        answers,
    )
}

#[tokio::test]
async fn interactive_fabricated_and_expired_options_never_become_remembered_grants() {
    use magistr_acp::AcpPermissionOptionKind as Kind;
    use magistr_application::sessions::AgentEvent;
    let state = state();
    let (mut permissions, mut events, answers) = interactive();
    let (request, reply) = request(&[("always", Kind::AllowAlways)]);
    permissions.answer(Some(request), &state);
    assert!(matches!(
        events.try_recv(),
        Ok(AgentEvent::PermissionRequest { id: 1, .. })
    ));
    permissions.reply(1, "fabricated".into(), &state);
    assert!(matches!(reply.await, Ok(AcpPermissionResponse::Cancelled)));
    assert!(matches!(
        events.try_recv(),
        Ok(AgentEvent::PermissionCancelled { id: 1 })
    ));
    let (request, reply) = self::request(&[("always", Kind::AllowAlways)]);
    permissions.answer(Some(request), &state);
    events.try_recv().unwrap();
    drop(reply);
    permissions.reply(2, "always".into(), &state);
    assert!(matches!(
        events.try_recv(),
        Ok(AgentEvent::PermissionCancelled { id: 2 })
    ));
    assert!(answers.lock().unwrap().is_empty());
    assert!(permissions.pending.is_empty());
}

#[test]
fn permission_observation_carries_the_action_the_user_is_answering() {
    let state = state();
    let (mut permissions, mut events, _) = interactive();
    let (mut request, _reply) = request(&[("allow", AcpPermissionOptionKind::AllowOnce)]);
    request.tool_input = serde_json::json!({"command":"remove selected.txt"});
    permissions.answer(Some(request), &state);
    let AgentEvent::PermissionRequest { detail, .. } = events.try_recv().unwrap() else {
        panic!("interactive request must be observable");
    };
    assert!(detail.contains("remove selected.txt"), "{detail}");
}

#[tokio::test]
async fn incomplete_shell_requests_wait_for_an_offered_user_choice() {
    use crate::permissions::{PermissionProfile, SessionPermissionPolicy};
    use magistr_engine::policy::RiskTier;

    for profile in [
        PermissionProfile::Cautious,
        PermissionProfile::Standard,
        PermissionProfile::Trusted,
    ] {
        for input in [
            serde_json::Value::Null,
            serde_json::json!({}),
            serde_json::json!({"command": ""}),
            serde_json::json!({"command": " \t\n"}),
            serde_json::json!({"command": 42}),
        ] {
            let state = state();
            let policy = SessionPermissionPolicy::new(profile, vec![], vec![], vec![], false);
            let (events, mut receiver) = crate::agent::observation::channel(
                crate::agent::observation::ObservationCapacity::DEFAULT,
            );
            let mut permissions = Permissions::interactive(None, Box::new(policy), events);
            let (mut request, mut reply) = request(&[
                ("deny", AcpPermissionOptionKind::RejectOnce),
                ("allow", AcpPermissionOptionKind::AllowOnce),
            ]);
            request.tool_title = "Bash".into();
            request.tool_kind = Some(agent_client_protocol::schema::v1::ToolKind::Execute);
            let expected_detail = input.to_string();
            request.tool_input = input;
            permissions.answer(Some(request), &state);

            let AgentEvent::PermissionRequest {
                id,
                title,
                detail,
                choices,
                risk,
                ..
            } = receiver.try_recv().expect("incomplete command must ask")
            else {
                panic!("expected a permission request");
            };
            assert_eq!(title, "Bash");
            assert_eq!(detail, expected_detail);
            assert_eq!(risk, RiskTier::High);
            assert_eq!(choices.len(), 2);
            assert_eq!(choices[0].option_id, "deny");
            assert_eq!(choices[1].option_id, "allow");
            assert!(
                matches!(
                    reply.try_recv(),
                    Err(tokio::sync::oneshot::error::TryRecvError::Empty)
                ),
                "must await the user's choice: {profile} {expected_detail}"
            );
            permissions.reply(id, "allow".into(), &state);
            assert!(matches!(
                reply.await,
                Ok(AcpPermissionResponse::Selected(id)) if id == "allow"
            ));
            assert!(permissions.pending.is_empty());
        }
    }
}

#[tokio::test]
async fn unattended_incomplete_shell_requests_are_refused() {
    for input in [
        serde_json::Value::Null,
        serde_json::json!({}),
        serde_json::json!({"command": ""}),
        serde_json::json!({"command": " \t\n"}),
        serde_json::json!({"command": false}),
    ] {
        let (mut permissions, _sender) = permissions(NoninteractivePolicy::Refuse);
        let (mut request, reply) = request(&[("allow", AcpPermissionOptionKind::AllowOnce)]);
        request.tool_title = "Bash".into();
        request.tool_kind = Some(agent_client_protocol::schema::v1::ToolKind::Execute);
        request.tool_input = input;
        permissions.answer(Some(request), &state());
        assert!(matches!(reply.await, Ok(AcpPermissionResponse::Cancelled)));
    }
}

#[tokio::test]
async fn interactive_always_answer_does_not_bypass_the_next_policy_ask() {
    use magistr_acp::AcpPermissionOptionKind as Kind;
    use magistr_application::sessions::{AgentEvent, PermissionKind};
    let state = state();
    let (mut permissions, mut events, answers) = interactive();
    let (first, reply) = request(&[("always", Kind::AllowAlways)]);
    permissions.answer(Some(first), &state);
    events.try_recv().unwrap();
    permissions.reply(1, "always".into(), &state);
    assert!(matches!(reply.await, Ok(AcpPermissionResponse::Selected(id)) if id == "always"));
    assert_eq!(*answers.lock().unwrap(), vec![PermissionKind::AllowAlways]);
    let (second, mut reply) = request(&[("once", Kind::AllowOnce)]);
    permissions.answer(Some(second), &state);
    assert!(matches!(
        events.try_recv(),
        Ok(AgentEvent::PermissionRequest { id: 2, .. })
    ));
    assert!(matches!(
        reply.try_recv(),
        Err(tokio::sync::oneshot::error::TryRecvError::Empty)
    ));
}

#[tokio::test]
async fn interactive_lapse_removes_the_matching_card_and_late_answer_is_quiet() {
    use magistr_application::sessions::AgentEvent;
    let state = state();
    let (mut permissions, mut events, answers) = interactive();
    let (request, reply) = request(&[("allow", magistr_acp::AcpPermissionOptionKind::AllowOnce)]);
    permissions.answer(Some(request), &state);
    events.try_recv().unwrap();
    permissions.lapse(&serde_json::json!({"tool_call_id": "unknown", "timeout_secs": 120}));
    assert!(events.try_recv().is_err());
    permissions.lapse(&serde_json::json!({"tool_call_id": "headless-call", "timeout_secs": 120}));
    assert!(matches!(
        events.try_recv(),
        Ok(AgentEvent::PermissionLapsed {
            id: 1,
            after_secs: 120,
            ..
        })
    ));
    assert!(matches!(reply.await, Ok(AcpPermissionResponse::Cancelled)));
    permissions.reply(1, "allow".into(), &state);
    assert!(events.try_recv().is_err());
    assert!(answers.lock().unwrap().is_empty());
}

#[tokio::test]
async fn interactive_cancel_refuses_pending_late_and_stale_answers_before_fresh_turn() {
    use magistr_application::sessions::AgentEvent;
    let state = state();
    let (mut permissions, mut events, answers) = interactive();
    let options = [("allow", magistr_acp::AcpPermissionOptionKind::AllowOnce)];
    let (first, reply) = request(&options);
    permissions.answer(Some(first), &state);
    events.try_recv().unwrap();
    state.lock().unwrap().snapshot.cancellation_requested = true;
    // Even a previously enqueued answer cannot acquire a grant after the
    // synchronous cancellation reservation has returned to the client.
    permissions.reply(1, "allow".into(), &state);
    assert!(matches!(reply.await, Ok(AcpPermissionResponse::Cancelled)));
    assert!(matches!(
        events.try_recv(),
        Ok(AgentEvent::PermissionCancelled { id: 1 })
    ));
    let (late, reply) = request(&options);
    permissions.answer(Some(late), &state);
    assert!(matches!(reply.await, Ok(AcpPermissionResponse::Cancelled)));
    assert!(events.try_recv().is_err());
    assert!(answers.lock().unwrap().is_empty());
    {
        let mut inner = state.lock().unwrap();
        inner.snapshot.active_turn = Some(2);
        inner.snapshot.cancellation_requested = false;
    }
    let (fresh, reply) = request(&options);
    permissions.answer(Some(fresh), &state);
    assert!(matches!(
        events.try_recv(),
        Ok(AgentEvent::PermissionRequest { id: 2, .. })
    ));
    drop(permissions);
    assert!(matches!(reply.await, Ok(AcpPermissionResponse::Cancelled)));
    assert!(matches!(
        events.try_recv(),
        Ok(AgentEvent::PermissionCancelled { id: 2 })
    ));
}

#[tokio::test]
async fn headless_and_interactive_policy_answers_select_only_the_exact_once_kind() {
    use magistr_acp::AcpPermissionOptionKind as Kind;
    use magistr_application::sessions::PolicyDecision;
    for decision in [PolicyDecision::Allow, PolicyDecision::Deny] {
        for options in [
            vec![("allow", Kind::AllowOnce), ("reject", Kind::RejectOnce)],
            vec![("always", Kind::AllowAlways)],
            vec![("never", Kind::RejectAlways)],
            vec![],
        ] {
            let state = state();
            let mut responses = Vec::new();
            for is_interactive in [false, true] {
                let policy = Box::new(RecordingPolicy {
                    decision,
                    answers: Default::default(),
                });
                let (events, _receiver) = crate::agent::observation::channel(
                    crate::agent::observation::ObservationCapacity::DEFAULT,
                );
                let mut permissions = if is_interactive {
                    Permissions::interactive(None, policy, events)
                } else {
                    Permissions::new(None, Some((policy, NoninteractivePolicy::Refuse)))
                };
                let (request, reply) = request(&options);
                permissions.answer(Some(request), &state);
                responses.push(format!("{:?}", reply.await.unwrap()));
            }
            assert_eq!(
                responses[0], responses[1],
                "policy and exact option mapping agree across clients"
            );
            let expected = match decision {
                PolicyDecision::Allow => Kind::AllowOnce,
                PolicyDecision::Deny => Kind::RejectOnce,
                PolicyDecision::Ask(_) => unreachable!(),
            };
            if !options.iter().any(|(_, kind)| *kind == expected) {
                assert_eq!(responses[0], "Cancelled");
            }
        }
    }
}
