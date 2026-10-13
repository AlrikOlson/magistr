use super::*;

fn address() -> SessionAddress {
    SessionAddress {
        project: "project".into(),
        workspace: "/workspace".into(),
        session: "one".into(),
    }
}

#[test]
fn drafts_and_answers_advance_the_same_observation_sequence() {
    let owner = std::sync::Arc::new(SessionObservation::new(address()));
    let before = owner.snapshot().sequence;
    owner.draft("draft".into());
    assert!(owner.snapshot().sequence > before);
    owner.send(Msg::Agent(AgentEvent::PermissionRequest {
        detail: String::new(),
        id: 7,
        title: "Read".into(),
        choices: Vec::new(),
        edit: false,
        risk: magistr_domain::policy::RiskTier::Low,
        plan: None,
    }));
    let pending = owner.snapshot().sequence;
    owner.permission_answered(7);
    let snapshot = owner.snapshot();
    assert!(snapshot.sequence > pending);
    assert_eq!(snapshot.pending_requests().count(), 0);
    owner.send(Msg::CommandFailed {
        cmd: magistr_cockpit::cockpit::Cmd::Permission {
            id: 7,
            option_id: "allow".into(),
        },
        error: "delivery failed".into(),
    });
    assert_eq!(owner.snapshot().pending_requests().count(), 1);
}

#[test]
fn current_session_facts_survive_retained_transcript_rotation() {
    let owner = std::sync::Arc::new(SessionObservation::new(address()));
    owner.send(Msg::Agent(AgentEvent::Initialized {
        session_id: "provider".into(),
        model: Some("known model".into()),
    }));
    owner.send(Msg::InputAccepted("hello".into()));
    for _ in 0..MAX_MESSAGES + 5 {
        owner.send(Msg::Agent(AgentEvent::Text("reply".into())));
    }
    let snapshot = owner.snapshot();
    assert_eq!(snapshot.model.as_deref(), Some("known model"));
    assert_eq!(snapshot.status, SessionStatus::Working);
    owner.send(Msg::Agent(AgentEvent::TurnEnded(
        magistr_application::sessions::TurnEnd::EndTurn,
    )));
    assert_eq!(owner.snapshot().status, SessionStatus::Finished);
}

#[tokio::test]
async fn disconnected_readers_do_not_close_the_owner_and_reconnect_gets_a_reseed() {
    let owner = std::sync::Arc::new(SessionObservation::new(address()));
    drop(owner.subscribe());
    for _ in 0..MAX_MESSAGES + 20 {
        assert!(owner.send(Msg::Agent(AgentEvent::Text("text".into()))));
    }
    let mut reader = owner.subscribe();
    let snapshot = reader.snapshot();
    assert_eq!(snapshot.sequence, (MAX_MESSAGES + 20) as u64);
    assert_eq!(snapshot.messages.len(), MAX_MESSAGES);
    assert_eq!(snapshot.dropped, 20);
    assert!(snapshot.since(0).is_none());
    assert!(matches!(
        snapshot.replay().first(),
        Some(Msg::Agent(AgentEvent::Lagged { items: 20, .. }))
    ));
    owner.stop();
    assert!(reader.changed().await.unwrap().closed);
    assert!(!owner.send(Msg::Agent(AgentEvent::Text("late".into()))));
}

#[test]
fn bounded_bytes_and_pending_permission_survive_transcript_rotation() {
    let owner = std::sync::Arc::new(SessionObservation::new(address()));
    let request = Msg::Agent(AgentEvent::PermissionRequest {
        detail: String::new(),
        id: 7,
        title: "Read a file".into(),
        choices: Vec::new(),
        edit: false,
        risk: magistr_domain::policy::RiskTier::Low,
        plan: None,
    });
    owner.send(request);
    owner.send(Msg::Agent(AgentEvent::Text("x".repeat(MAX_BYTES))));
    let snapshot = owner.snapshot();
    assert!(snapshot.bytes <= MAX_BYTES);
    assert!(
        snapshot.since(0).is_none(),
        "an oversized delta remains an explicit gap when retention is empty"
    );
    assert!(snapshot.pending.contains_key(&7));
    assert!(
        snapshot
            .replay()
            .iter()
            .any(|msg| matches!(msg, Msg::Agent(AgentEvent::PermissionRequest { id: 7, .. })))
    );
    owner.permission_answered(7);
    assert!(owner.is_answered(7));
    owner.send(Msg::Agent(AgentEvent::PermissionCancelled { id: 7 }));
    assert!(!owner.snapshot().pending.contains_key(&7));
}
#[test]
fn usage_survives_rotation_and_too_many_requests_have_a_bounded_explicit_outcome() {
    let owner = std::sync::Arc::new(SessionObservation::new(address()));
    owner.send(Msg::Agent(AgentEvent::Usage {
        context: Some(magistr_application::sessions::ContextUsage {
            used: 100,
            size: 1000,
        }),
        cost_cents: Some(42),
        elapsed_secs: Some(10),
    }));
    for _ in 0..MAX_MESSAGES + 1 {
        owner.send(Msg::Agent(AgentEvent::Text("stream".into())));
    }
    assert!(owner.snapshot().replay().iter().any(|msg| matches!(
        msg,
        Msg::Agent(AgentEvent::Usage {
            cost_cents: Some(42),
            ..
        })
    )));
    for id in 0..MAX_REQUESTS as u64 {
        assert!(owner.send(Msg::Agent(AgentEvent::PermissionRequest {
            detail: String::new(),
            id,
            title: "Read".into(),
            choices: Vec::new(),
            edit: false,
            risk: magistr_domain::policy::RiskTier::Low,
            plan: None
        })));
    }
    assert!(!owner.send(Msg::Agent(AgentEvent::PermissionRequest {
        detail: String::new(),
        id: MAX_REQUESTS as u64,
        title: "Too many".into(),
        choices: Vec::new(),
        edit: false,
        risk: magistr_domain::policy::RiskTier::Low,
        plan: None
    })));
    let snapshot = owner.snapshot();
    assert!(snapshot.closed);
    assert_eq!(snapshot.pending.len(), MAX_REQUESTS);
    assert!(snapshot.replay().iter().any(|msg| matches!(msg, Msg::Agent(AgentEvent::Error(error)) if error.contains("too many requests"))));
}
#[test]
fn rejected_messages_remain_drafts_after_rotation_without_replacing_newer_input() {
    let owner = std::sync::Arc::new(SessionObservation::new(address()));
    owner.send(Msg::Agent(AgentEvent::PromptRejected {
        text: "unsent".into(),
        error: "provider unavailable".into(),
    }));
    for _ in 0..MAX_MESSAGES + 1 {
        owner.send(Msg::Agent(AgentEvent::Text("other stream".into())));
    }
    assert_eq!(owner.snapshot().seed.draft.as_deref(), Some("unsent"));
    owner.draft("newer input".into());
    owner.send(Msg::Agent(AgentEvent::PromptRejected {
        text: "old".into(),
        error: "closed".into(),
    }));
    assert_eq!(owner.snapshot().seed.draft.as_deref(), Some("newer input"));
}

#[tokio::test]
async fn checking_closure_does_not_consume_unread_observations() {
    let owner = std::sync::Arc::new(SessionObservation::new(address()));
    let mut reader = owner.subscribe();
    owner.send(Msg::Agent(AgentEvent::Text("unread".into())));
    assert!(!reader.is_closed());
    let snapshot = tokio::time::timeout(std::time::Duration::from_millis(100), reader.changed())
        .await
        .expect("status inspection must not consume the update")
        .unwrap();
    assert_eq!(snapshot.sequence, 1);
    owner.stop();
    assert!(reader.is_closed());
}

#[test]
fn finished_and_closed_sessions_keep_reported_spending_for_reconnected_readers() {
    let owner = std::sync::Arc::new(SessionObservation::new(address()));
    owner.send(Msg::Agent(AgentEvent::Usage {
        context: Some(magistr_application::sessions::ContextUsage { used: 8, size: 100 }),
        cost_cents: Some(25),
        elapsed_secs: None,
    }));
    owner.send(Msg::Agent(AgentEvent::TurnEnded(
        magistr_application::sessions::TurnEnd::EndTurn,
    )));
    assert!(matches!(
        owner.snapshot().latest_usage(),
        Some(Msg::Agent(AgentEvent::Usage {
            cost_cents: Some(25),
            ..
        }))
    ));
    owner.stop();
    let mut reader = owner.subscribe();
    let snapshot = reader.snapshot();
    assert!(snapshot.closed);
    assert!(matches!(
        snapshot.latest_usage(),
        Some(Msg::Agent(AgentEvent::Usage {
            cost_cents: Some(25),
            ..
        }))
    ));
}
