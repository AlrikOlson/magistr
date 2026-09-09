use std::path::PathBuf;

use tokio::sync::oneshot;

use super::*;
use crate::agent::AskEverything;

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
        allowed: HashSet::new(),
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
