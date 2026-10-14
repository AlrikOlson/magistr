//! Context identities survive parking, late events and queued command dispatch.

use super::*;
use crate::channel::{Arrivals, ObservationSink};

#[derive(Debug)]
enum Observation {
    Activated(String),
    Message(Option<String>, Box<Msg>),
}

#[derive(Debug)]
struct Observations(mpsc::UnboundedSender<Observation>);

impl ObservationSink for Observations {
    fn send(&self, message: Msg) -> bool {
        self.0
            .send(Observation::Message(None, Box::new(message)))
            .is_ok()
    }

    fn activate(&self, id: &str) -> Result<(), String> {
        self.0
            .send(Observation::Activated(id.into()))
            .map_err(|error| error.to_string())
    }

    fn send_for(&self, id: &str, message: Msg) -> bool {
        self.0
            .send(Observation::Message(Some(id.into()), Box::new(message)))
            .is_ok()
    }

    fn is_closed(&self) -> bool {
        self.0.is_closed()
    }

    fn closed(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(self.0.closed())
    }
}

fn observed() -> (Inbox, mpsc::UnboundedReceiver<Observation>) {
    let (sender, receiver) = mpsc::unbounded_channel();
    (Inbox::observed(Arc::new(Observations(sender))), receiver)
}

fn addressed(id: &str, command: Cmd) -> Cmd {
    Cmd::ForConversation {
        id: id.into(),
        command: Box::new(command),
    }
}

async fn message(receiver: &mut mpsc::UnboundedReceiver<Observation>) -> (String, Msg) {
    loop {
        match next(receiver).await {
            Observation::Message(Some(id), message) => return (id, *message),
            Observation::Activated(id) => assert!(id.starts_with("logical-")),
            Observation::Message(None, message) => {
                panic!("unbound conversation event: {message:?}")
            }
        }
    }
}

async fn accepted(receiver: &mut mpsc::UnboundedReceiver<Observation>, expected: &str) {
    loop {
        let (id, message) = message(receiver).await;
        assert_eq!(id, expected);
        if matches!(message, Msg::InputAccepted(_)) {
            break;
        }
    }
}

#[tokio::test]
async fn scope_survives_taps_stamps_and_activation_of_another_context() {
    let (inbox, mut events) = observed();
    let (tap, mut tapped) = mpsc::unbounded_channel();
    let arrivals = Arrivals::new();
    let producer = inbox
        .for_conversation("logical-a")
        .tap(tap)
        .stamped(arrivals.clone());
    inbox.activate("logical-b").unwrap();
    assert!(matches!(next(&mut events).await, Observation::Activated(id) if id == "logical-b"));
    let late = Msg::Agent(AgentEvent::Text("late reply".into()));
    assert!(producer.send(late.clone()));
    assert!(
        matches!(next(&mut events).await, Observation::Message(Some(id), message) if id == "logical-a" && *message == late)
    );
    assert_eq!(next(&mut tapped).await, late);
    assert_eq!(arrivals.len(), 1);
}

#[tokio::test]
async fn parked_contexts_keep_late_events_and_duplicate_permission_ids_separate() {
    let (inbox, mut observations) = observed();
    let (commands, receiver) = Commands::channel();
    let (opened, mut connections) = mpsc::unbounded_channel();
    let (handle, mut conversation) = controlled(SessionPurpose::Conversation);
    let task = InteractiveSession::new(
        move |purpose, _| {
            let (handle, observed) = controlled(purpose);
            opened.send(observed).unwrap();
            async { Ok(handle) }
        },
        inbox,
        None,
        None,
        None,
    )
    .connected(handle, false)
    .spawn(receiver);
    assert!(
        matches!(next(&mut observations).await, Observation::Activated(id) if id == "logical-conversation")
    );

    commands
        .send(addressed(
            "logical-conversation",
            Cmd::RoadmapPrompt("Plan".into()),
        ))
        .unwrap();
    let mut planning = next(&mut connections).await;
    assert_eq!(next(&mut planning.prompts).await, "Plan");
    accepted(&mut observations, "logical-roadmap").await;

    let permission = Cmd::Permission {
        id: 7,
        option_id: "allow".into(),
    };
    commands
        .send(addressed("logical-conversation", permission.clone()))
        .unwrap();
    assert!(
        matches!(message(&mut observations).await, (id, Msg::CommandFailed { cmd, .. }) if id == "logical-conversation" && cmd == permission)
    );
    assert!(conversation.calls.try_recv().is_err());
    assert!(planning.calls.try_recv().is_err());
    commands
        .send(addressed("logical-roadmap", permission.clone()))
        .unwrap();
    assert_eq!(next(&mut planning.calls).await, permission);

    conversation
        .events
        .send(AgentEvent::Text("A late response".into()))
        .unwrap();
    assert!(
        matches!(message(&mut observations).await, (id, Msg::Agent(AgentEvent::Text(text))) if id == "logical-conversation" && text == "A late response")
    );
    planning
        .events
        .send(AgentEvent::TurnEnded(TurnEnd::EndTurn))
        .unwrap();
    assert!(
        matches!(message(&mut observations).await, (id, Msg::Agent(AgentEvent::TurnEnded(_))) if id == "logical-roadmap")
    );

    commands.send(Cmd::Prompt("Continue A".into())).unwrap();
    assert_eq!(next(&mut conversation.prompts).await, "Continue A");
    accepted(&mut observations, "logical-conversation").await;
    commands
        .send(addressed("logical-roadmap", permission.clone()))
        .unwrap();
    assert!(
        matches!(message(&mut observations).await, (id, Msg::CommandFailed { cmd, .. }) if id == "logical-roadmap" && cmd == permission)
    );
    commands
        .send(addressed("logical-conversation", permission.clone()))
        .unwrap();
    assert_eq!(next(&mut conversation.calls).await, permission);
    assert!(planning.calls.try_recv().is_err());

    conversation
        .events
        .send(AgentEvent::TurnEnded(TurnEnd::EndTurn))
        .unwrap();
    assert!(matches!(
        message(&mut observations).await,
        (_, Msg::Agent(AgentEvent::TurnEnded(_)))
    ));
    commands
        .send(Cmd::RoadmapPrompt("Continue B".into()))
        .unwrap();
    assert_eq!(next(&mut planning.prompts).await, "Continue B");
    accepted(&mut observations, "logical-roadmap").await;
    assert!(
        connections.try_recv().is_err(),
        "both logical contexts retain their own provider"
    );
    commands.send(Cmd::Quit).unwrap();
    task.await.unwrap().unwrap();
}

#[tokio::test]
async fn commands_waiting_across_a_connection_switch_never_reach_the_new_owner() {
    let (inbox, mut observations) = observed();
    let (commands, receiver) = Commands::channel();
    let (opened, mut connections) = mpsc::unbounded_channel();
    let (release, ready) = tokio::sync::oneshot::channel();
    let ready = Arc::new(std::sync::Mutex::new(Some(ready)));
    let (handle, _conversation) = controlled(SessionPurpose::Conversation);
    let task = InteractiveSession::new(
        move |purpose, _| {
            let (handle, observed) = controlled(purpose);
            opened.send(observed).unwrap();
            let ready = ready.lock().unwrap().take().unwrap();
            async move {
                ready.await.unwrap();
                Ok(handle)
            }
        },
        inbox,
        None,
        None,
        None,
    )
    .connected(handle, false)
    .spawn(receiver);
    next(&mut observations).await;
    commands
        .send(addressed(
            "logical-conversation",
            Cmd::RoadmapPrompt("Plan".into()),
        ))
        .unwrap();
    let mut planning = next(&mut connections).await;
    let stale = Cmd::Permission {
        id: 7,
        option_id: "allow".into(),
    };
    commands
        .send(addressed("logical-conversation", stale.clone()))
        .unwrap();
    release.send(()).unwrap();
    let mut rejected = false;
    let mut admitted = false;
    while !rejected || !admitted {
        let (id, event) = message(&mut observations).await;
        match event {
            Msg::CommandFailed { cmd, .. } => {
                assert_eq!(id, "logical-conversation");
                assert_eq!(cmd, stale);
                rejected = true;
            }
            Msg::InputAccepted(_) => {
                assert_eq!(id, "logical-roadmap");
                admitted = true;
            }
            Msg::RoadmapRequestStarted => assert_eq!(id, "logical-roadmap"),
            other => panic!("unexpected event: {other:?}"),
        }
    }
    assert_eq!(next(&mut planning.prompts).await, "Plan");
    assert!(planning.calls.try_recv().is_err());
    commands
        .send(addressed("logical-roadmap", stale.clone()))
        .unwrap();
    assert_eq!(next(&mut planning.calls).await, stale);
    commands.send(Cmd::Quit).unwrap();
    task.await.unwrap().unwrap();
}

#[tokio::test]
async fn addressed_sends_and_reconnect_keep_the_opening_planning_purpose() {
    let (inbox, mut observations) = observed();
    let (commands, receiver) = Commands::channel();
    let (opened, mut connections) = mpsc::unbounded_channel();
    let (handle, mut planning) = controlled(SessionPurpose::Roadmap);
    let task = InteractiveSession::new(
        move |purpose, resume| {
            let (handle, observed) = controlled(purpose);
            opened.send((purpose, resume, observed)).unwrap();
            async { Ok(handle) }
        },
        inbox,
        None,
        None,
        None,
    )
    .connected(handle, false)
    .spawn(receiver);
    assert!(
        matches!(next(&mut observations).await, Observation::Activated(id) if id == "logical-roadmap")
    );
    commands
        .send(addressed(
            "logical-roadmap",
            Cmd::Prompt("Continue planning".into()),
        ))
        .unwrap();
    assert_eq!(next(&mut planning.prompts).await, "Continue planning");
    accepted(&mut observations, "logical-roadmap").await;
    commands
        .send(addressed(
            "logical-roadmap",
            Cmd::Prompt("Queued planning".into()),
        ))
        .unwrap();
    assert_eq!(next(&mut planning.prompts).await, "Queued planning");
    accepted(&mut observations, "logical-roadmap").await;
    assert!(connections.try_recv().is_err());

    for event in [
        AgentEvent::Initialized {
            session_id: "provider-planning".into(),
            model: None,
        },
        AgentEvent::Activity(magistr_application::sessions::SessionActivity {
            active: false,
            queued_turns: 0,
            configuration_in_progress: false,
            cancellation_requested: false,
            can_resume: Some(true),
            can_send_images: Some(false),
        }),
        AgentEvent::Error("lost connection".into()),
    ] {
        planning.events.send(event).unwrap();
        assert_eq!(message(&mut observations).await.0, "logical-roadmap");
    }
    commands
        .send(addressed("logical-roadmap", Cmd::Reconnect))
        .unwrap();
    let (purpose, resume, mut restored) = next(&mut connections).await;
    assert_eq!(purpose, SessionPurpose::Roadmap);
    assert_eq!(resume.as_deref(), Some("provider-planning"));
    assert!(restored.prompts.try_recv().is_err());
    restored
        .events
        .send(AgentEvent::Text("old reply".into()))
        .unwrap();
    restored
        .events
        .send(AgentEvent::Initialized {
            session_id: "provider-planning".into(),
            model: None,
        })
        .unwrap();
    assert!(
        matches!(message(&mut observations).await, (id, Msg::Agent(AgentEvent::Initialized { .. })) if id == "logical-roadmap")
    );
    commands.send(Cmd::Quit).unwrap();
    task.await.unwrap().unwrap();
}

#[tokio::test]
async fn restored_transport_acknowledges_new_input_while_omitting_history_replay() {
    let (inbox, mut observations) = observed();
    let (commands, receiver) = Commands::channel();
    let (opened, mut connections) = mpsc::unbounded_channel();
    let (handle, opening) = controlled(SessionPurpose::Conversation);
    let task = InteractiveSession::new(
        move |purpose, resume| {
            assert_eq!(resume.as_deref(), Some("provider-a"));
            let (handle, observed) = controlled(purpose);
            opened.send(observed).unwrap();
            async { Ok(handle) }
        },
        inbox,
        None,
        None,
        None,
    )
    .connected(handle, false)
    .spawn(receiver);
    next(&mut observations).await;
    opening
        .events
        .send(AgentEvent::Initialized {
            session_id: "provider-a".into(),
            model: None,
        })
        .unwrap();
    message(&mut observations).await;
    opening
        .events
        .send(AgentEvent::Error("connection lost".into()))
        .unwrap();
    message(&mut observations).await;
    commands
        .send(addressed(
            "logical-conversation",
            Cmd::Prompt("new input".into()),
        ))
        .unwrap();
    let mut restored = next(&mut connections).await;
    assert_eq!(next(&mut restored.prompts).await, "new input");
    accepted(&mut observations, "logical-conversation").await;
    restored
        .events
        .send(AgentEvent::Text("old response".into()))
        .unwrap();
    restored
        .events
        .send(AgentEvent::Initialized {
            session_id: "provider-a".into(),
            model: None,
        })
        .unwrap();
    assert!(
        matches!(message(&mut observations).await, (id, Msg::Agent(AgentEvent::Initialized { .. })) if id == "logical-conversation")
    );
    commands.send(Cmd::Quit).unwrap();
    task.await.unwrap().unwrap();
}
