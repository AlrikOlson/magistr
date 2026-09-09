use std::path::PathBuf;

use tokio::sync::oneshot;

use super::*;
use crate::agent::AskEverything;

struct RecordingPolicy(Arc<Mutex<Vec<PermissionKind>>>);

struct FixedPolicy(PolicyDecision);

impl SessionPolicy for FixedPolicy {
    fn decide(&mut self, _request: &AcpPermissionRequest) -> PolicyDecision {
        self.0
    }
}

impl SessionPolicy for RecordingPolicy {
    fn decide(&mut self, _request: &AcpPermissionRequest) -> PolicyDecision {
        PolicyDecision::Ask(magistr_engine::policy::RiskTier::Medium)
    }

    fn answered(&mut self, _request: &AcpPermissionRequest, kind: PermissionKind) {
        self.0.lock().expect("recorded answers").push(kind);
    }
}

/// A session over an unconnected transport: enough to drive `request`
/// and `event` — the relay's own folds — without an agent.
fn session(inbox: Inbox) -> Session {
    Session {
        transport: AcpTransport::new_with_args(PathBuf::from("fake-acp-agent"), Vec::new(), None)
            .expect("a transport"),
        inbox,
        options: SessionOptions::new(),
        policy: Box::new(AskEverything),
        workbench: None,
        engine: None,
        checkpoints: None,
        pending: HashMap::new(),
        next_id: 1,
        cancelled: false,
        tool_started: HashMap::new(),
        started: Instant::now(),
    }
}

/// A request the policy cannot decide, with its reply channel held.
fn ask(
    call: &str,
) -> (
    AcpPermissionRequest,
    oneshot::Receiver<AcpPermissionResponse>,
) {
    let (response_tx, response_rx) = oneshot::channel();
    let request = AcpPermissionRequest {
        tool_call_id: call.to_owned(),
        tool_title: "Bash".to_owned(),
        tool_input: serde_json::Value::Null,
        tool_kind: None,
        options: vec![
            AcpPermissionOption {
                option_id: "allow".to_owned(),
                kind: AcpPermissionOptionKind::AllowOnce,
            },
            AcpPermissionOption {
                option_id: "reject".to_owned(),
                kind: AcpPermissionOptionKind::RejectOnce,
            },
        ],
        response_tx,
    };
    (request, response_rx)
}

/// The transport's own word that `call` timed out and was denied.
fn lapsed(call: &str) -> magistr_acp::Result<AcpEvent> {
    Ok(AcpEvent::System {
        subtype: "permission_lapsed".to_owned(),
        data: serde_json::json!({
            "tool_call_id": call,
            "tool_title": "Bash",
            "timeout_secs": 120,
        }),
    })
}

/// The transport denied a request nobody answered: the session finds
/// the card by the call's id, forgets its reply channel, and posts the
/// lapse under the id the cockpit knows the card by.
#[tokio::test]
async fn a_lapsed_request_leaves_the_cockpit_by_its_own_id() {
    let (inbox, mut rx) = Inbox::channel();
    let mut session = session(inbox);
    let (request, _reply) = ask("tc-7");
    assert!(session.request(request));
    let id = match rx.try_recv() {
        Ok(Msg::Agent(AgentEvent::PermissionRequest { id, .. })) => id,
        other => panic!("the ask reached the inbox: {other:?}"),
    };
    assert!(session.event(lapsed("tc-7")).await);
    assert!(matches!(
        rx.try_recv(),
        Ok(Msg::Agent(AgentEvent::PermissionLapsed { id: lapsed_id, title, after_secs: 120 }))
            if lapsed_id == id && title == "Bash"
    ));
    assert!(session.pending.is_empty(), "the reply channel is forgotten");
    // A late answer for the lapsed id is nobody's.
    session.command(Cmd::Permission {
        id,
        option_id: "allow".to_owned(),
    });
    assert!(rx.try_recv().is_err(), "nothing else is posted");
}

/// A lapse for a call the cockpit never saw — the policy answered it,
/// or it was answered already — posts nothing, not even a System line.
#[tokio::test]
async fn a_lapse_for_an_unknown_call_is_quiet() {
    let (inbox, mut rx) = Inbox::channel();
    let mut session = session(inbox);
    assert!(session.event(lapsed("tc-9")).await);
    assert!(rx.try_recv().is_err(), "nothing posted");
}

/// Even an Always answer is the policy's decision to remember: a later
/// governance Ask must reach the cockpit again.
#[tokio::test]
async fn a_policy_ask_is_not_bypassed_by_an_earlier_always_answer() {
    let (inbox, mut rx) = Inbox::channel();
    let mut session = session(inbox);
    let (mut request, reply) = ask("first");
    request.options.push(AcpPermissionOption {
        option_id: "always".to_owned(),
        kind: AcpPermissionOptionKind::AllowAlways,
    });
    assert!(session.request(request));
    session.command(Cmd::Permission {
        id: 1,
        option_id: "always".to_owned(),
    });
    assert!(matches!(reply.await, Ok(AcpPermissionResponse::Selected(id)) if id == "always"));
    rx.try_recv().expect("the first permission card");

    let (request, mut reply) = ask("second");
    assert!(session.request(request));
    assert!(matches!(
        reply.try_recv(),
        Err(oneshot::error::TryRecvError::Empty)
    ));
    assert!(matches!(
        rx.try_recv(),
        Ok(Msg::Agent(AgentEvent::PermissionRequest { id: 2, .. }))
    ));
}

#[tokio::test]
async fn a_fabricated_permission_option_is_cancelled() {
    let (inbox, _rx) = Inbox::channel();
    let mut session = session(inbox);
    let answers = Arc::new(Mutex::new(Vec::new()));
    session.policy = Box::new(RecordingPolicy(Arc::clone(&answers)));
    let (request, reply) = ask("fabricated");
    assert!(session.request(request));
    session.command(Cmd::Permission {
        id: 1,
        option_id: "allow_everything".to_owned(),
    });
    assert!(matches!(reply.await, Ok(AcpPermissionResponse::Cancelled)));
    assert!(session.pending.is_empty());
    assert!(answers.lock().expect("recorded answers").is_empty());
}

#[tokio::test]
async fn an_answer_after_the_transport_dropped_its_wait_is_not_remembered() {
    let (inbox, _rx) = Inbox::channel();
    let mut session = session(inbox);
    let answers = Arc::new(Mutex::new(Vec::new()));
    session.policy = Box::new(RecordingPolicy(Arc::clone(&answers)));
    let (mut request, reply) = ask("expired");
    request.options.push(AcpPermissionOption {
        option_id: "always".to_owned(),
        kind: AcpPermissionOptionKind::AllowAlways,
    });
    assert!(session.request(request));
    drop(reply);
    session.command(Cmd::Permission {
        id: 1,
        option_id: "always".to_owned(),
    });
    assert!(answers.lock().expect("recorded answers").is_empty());
}

#[tokio::test]
async fn cancelling_answers_every_pending_request_and_forgets_late_choices() {
    let (inbox, _rx) = Inbox::channel();
    let mut session = session(inbox);
    let (first, first_reply) = ask("first");
    let (second, second_reply) = ask("second");
    assert!(session.request(first));
    assert!(session.request(second));
    session.command(Cmd::Cancel);
    assert!(
        session.pending.is_empty(),
        "cancelling clears pending cards"
    );
    assert!(matches!(
        first_reply.await,
        Ok(AcpPermissionResponse::Cancelled)
    ));
    assert!(matches!(
        second_reply.await,
        Ok(AcpPermissionResponse::Cancelled)
    ));
    session.command(Cmd::Permission {
        id: 1,
        option_id: "allow".to_owned(),
    });
    assert!(session.pending.is_empty());
}

#[tokio::test]
async fn requests_arriving_after_cancellation_wait_for_a_fresh_prompt() {
    for decision in [
        PolicyDecision::Allow,
        PolicyDecision::Ask(magistr_engine::policy::RiskTier::Medium),
    ] {
        let (inbox, mut rx) = Inbox::channel();
        let mut session = session(inbox);
        session.policy = Box::new(FixedPolicy(decision));
        session.command(Cmd::Cancel);
        rx.try_recv()
            .expect("unconnected transport reports cancellation error");
        let (request, mut reply) = ask("queued-before-cancel");
        assert!(session.request(request));
        assert!(matches!(
            reply.try_recv(),
            Ok(AcpPermissionResponse::Cancelled)
        ));
        assert!(session.pending.is_empty());
        assert!(
            rx.try_recv().is_err(),
            "a stale request never becomes a card"
        );

        session.command(Cmd::Prompt("new turn".to_owned()));
        rx.try_recv()
            .expect("unconnected transport reports prompt error");
        let (request, mut reply) = ask("fresh-turn");
        assert!(session.request(request));
        match decision {
            PolicyDecision::Allow => assert!(
                matches!(reply.try_recv(), Ok(AcpPermissionResponse::Selected(id)) if id == "allow")
            ),
            PolicyDecision::Ask(_) => assert!(matches!(
                rx.try_recv(),
                Ok(Msg::Agent(AgentEvent::PermissionRequest { .. }))
            )),
            PolicyDecision::Deny => unreachable!("allow and ask cases"),
        }
    }
}

#[tokio::test]
async fn dropping_the_session_answers_pending_requests_with_cancellation() {
    let (inbox, _rx) = Inbox::channel();
    let mut session = session(inbox);
    let (request, reply) = ask("shutdown");
    assert!(session.request(request));
    drop(session);
    assert!(matches!(reply.await, Ok(AcpPermissionResponse::Cancelled)));
}

#[tokio::test]
async fn transport_closure_cancels_pending_requests_before_runtime_shutdown() {
    let (inbox, _rx) = Inbox::channel();
    let mut session = session(inbox);
    let (request, reply) = ask("disconnect");
    assert!(session.request(request));
    let (event_tx, events) = tokio::sync::mpsc::unbounded_channel();
    let (_command_tx, commands) = Commands::channel();
    drop(event_tx);
    let task = tokio::spawn(session.run(Some(events), None, commands));
    let answer = tokio::time::timeout(std::time::Duration::from_secs(2), reply).await;
    task.abort();
    assert!(matches!(answer, Ok(Ok(AcpPermissionResponse::Cancelled))));
}

#[tokio::test]
async fn a_transport_error_cancels_pending_requests_before_channels_close() {
    let (inbox, _rx) = Inbox::channel();
    let mut session = session(inbox);
    let (request, mut reply) = ask("disconnect");
    assert!(session.request(request));
    assert!(
        session
            .event(Err(magistr_acp::AcpError::Transport(
                "connection closed".to_owned()
            )))
            .await
    );
    assert!(matches!(
        reply.try_recv(),
        Ok(AcpPermissionResponse::Cancelled)
    ));
    assert!(session.pending.is_empty());
}

#[test]
fn a_policy_answer_never_selects_an_incompatible_permission_kind() {
    let allow = [AcpPermissionOption {
        option_id: "allow".to_owned(),
        kind: AcpPermissionOptionKind::AllowOnce,
    }];
    assert!(matches!(
        select(&allow, AcpPermissionOptionKind::RejectOnce),
        AcpPermissionResponse::Cancelled
    ));
    let reject = [AcpPermissionOption {
        option_id: "reject".to_owned(),
        kind: AcpPermissionOptionKind::RejectOnce,
    }];
    assert!(matches!(
        select(&reject, AcpPermissionOptionKind::AllowOnce),
        AcpPermissionResponse::Cancelled
    ));
}
