use super::*;
use crate::agent::{AgentConfig, ResolvedSessionSpec, persistence::storage};
use crate::config::inputs::ProjectInputs;
use crate::orchestration::{interactive_session::interactive_session, loop_driver::Connector};
use magistr_application::sessions::{SessionOptions, SessionPurpose};
use magistr_cockpit::cockpit::Cmd;
use magistr_cockpit_runtime::channel::{Commands, Inbox};
use std::sync::Arc;
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

#[tokio::test]
async fn real_supervisors_bind_opening_input_and_keep_parked_drafts_through_reconnect() {
    let dir = tempfile::tempdir().unwrap();
    let inputs = ProjectInputs::from_lookup(dir.path(), |key| {
        (key == "XDG_DATA_HOME").then(|| dir.path().join("data").display().to_string())
    });
    let owner = Arc::new(SessionObservation::new(SessionAddress {
        project: "project".into(),
        workspace: dir.path().display().to_string(),
        session: "resident".into(),
    }));
    let (sent, mut events) = unbounded_channel();
    let inbox = Inbox::observed(owner.clone()).tap(sent);
    let (opened, mut connections) = unbounded_channel();
    let connector = Connector::new(move |config: AgentConfig| {
        let opened = opened.clone();
        async move {
            let spec = ResolvedSessionSpec::resolve_with_runtime(
                config.clone(),
                &crate::config::user_config::UserConfig::default(),
                crate::agent::AgentRuntime {
                    path: "controlled".into(),
                    args: vec![],
                    auth: crate::agent::AuthMethod::Implicit,
                    provider: "scripted".into(),
                },
            )?;
            spec.history()
                .begin(config.inputs(), &config.cwd, "scripted", spec.model())
                .await?;
            let (channel, agent) = provider();
            let mut transport =
                magistr_acp::AcpTransport::new_with_args("controlled".into(), vec![], None)
                    .unwrap()
                    .session_mode(spec.session_mode());
            transport
                .connect_over(channel)
                .await
                .map_err(|error| crate::error::MagistrError::Acp(error.to_string()))?;
            opened.send((config, agent)).unwrap();
            Ok((transport, spec))
        }
    })
    .observing(Some(owner.clone()));
    let config = AgentConfig::new("", dir.path()).with_inputs(&inputs);
    let (transport, spec) = connector.connect(config.clone()).await.unwrap();
    let handle = connector
        .start_session(
            transport,
            spec,
            SessionOptions::new().opening_prompt("opening A"),
        )
        .await
        .unwrap();
    let a = owner.snapshot().target().unwrap();
    let (history_a, agent_a) = connections.recv().await.unwrap();
    assert_eq!(a.conversation, history_a.history.id());
    assert!(
        owner.owns(&a.conversation),
        "binding precedes opening input"
    );
    let (commands, receiver) = Commands::channel();
    let task = interactive_session(config, connector, inbox.clone(), None)
        .connected(Box::new(handle), true)
        .spawn(receiver);
    turn(&mut events).await;
    assert!(owner.snapshot().replay().iter().any(
        |message| matches!(message, Msg::Agent(AgentEvent::Text(text)) if text == "new reply")
    ));
    owner
        .edit_draft_for(&a.conversation, 0, "A draft".into(), false)
        .unwrap();
    commands.send(Cmd::RoadmapPrompt("plan B".into())).unwrap();
    turn(&mut events).await;
    let b = owner.snapshot().target().unwrap();
    let (history_b, agent_b) = connections.recv().await.unwrap();
    assert_ne!(a.conversation, b.conversation);
    assert_eq!(history_b.history.purpose(), SessionPurpose::Roadmap);
    assert_eq!(
        owner.snapshot().conversation.unwrap().purpose,
        SessionPurpose::Roadmap
    );
    owner
        .edit_draft_for(&b.conversation, 0, "B draft".into(), false)
        .unwrap();
    // A renderer edit captured before the switch remains saveable to parked A.
    owner
        .edit_draft_for(&a.conversation, 1, "A late draft".into(), false)
        .unwrap();
    assert_eq!(owner.snapshot().seed.draft.as_deref(), Some("B draft"));
    inbox
        .for_conversation(&a.conversation)
        .send(Msg::Agent(AgentEvent::Notice("late A".into())));
    assert!(!owner.snapshot().replay().iter().any(
        |message| matches!(message, Msg::Agent(AgentEvent::Notice(text)) if text == "late A")
    ));
    let before = owner.snapshot().sequence;
    commands.send(Cmd::Prompt("return A".into())).unwrap();
    turn(&mut events).await;
    let restored_a = owner.snapshot();
    assert_eq!(restored_a.target().as_ref(), Some(&a));
    assert!(restored_a.sequence > before);
    assert!(restored_a.since(before).is_none());
    assert_eq!(restored_a.seed.draft.as_deref(), Some("A late draft"));
    assert!(restored_a.replay().iter().any(
        |message| matches!(message, Msg::Agent(AgentEvent::Notice(text)) if text == "late A")
    ));
    commands
        .send(Cmd::RoadmapPrompt("return B".into()))
        .unwrap();
    turn(&mut events).await;
    assert_eq!(owner.snapshot().target().as_ref(), Some(&b));
    assert_eq!(owner.snapshot().seed.draft.as_deref(), Some("B draft"));
    agent_b.abort();
    let _ = agent_b.await;
    loop {
        if matches!(next(&mut events).await, Msg::Agent(AgentEvent::Error(_))) {
            break;
        }
    }
    commands
        .send(Cmd::ForConversation {
            id: b.conversation.clone(),
            command: Box::new(Cmd::Prompt("reconnected B".into())),
        })
        .unwrap();
    turn(&mut events).await;
    let (reconnected, agent_b) = connections.recv().await.unwrap();
    assert_eq!(reconnected.history.id(), b.conversation);
    assert_eq!(reconnected.history.purpose(), SessionPurpose::Roadmap);
    assert_eq!(reconnected.resume.as_deref(), Some("provider"));
    assert_eq!(owner.snapshot().target().as_ref(), Some(&b));
    assert!(!owner.snapshot().replay().iter().any(
        |message| matches!(message, Msg::Agent(AgentEvent::Text(text)) if text.contains("old replay"))
    ));
    owner.flush_all().await.unwrap();
    drop(commands);
    task.await.unwrap().unwrap();
    owner.flush_all().await.unwrap();
    agent_a.abort();
    agent_b.abort();
    let conn = storage::open_readonly_db(&storage::sessions_db_path(&inputs).unwrap()).unwrap();
    assert_eq!(storage::count_sessions(&conn).unwrap(), 2);
    let saved_a = storage::get_session(&conn, &a.conversation)
        .unwrap()
        .unwrap();
    let saved_b = storage::get_session(&conn, &b.conversation)
        .unwrap()
        .unwrap();
    assert_eq!(saved_a.turns, Some(2));
    assert_eq!(saved_b.turns, Some(3));
    assert_eq!(saved_a.purpose, SessionPurpose::Conversation);
    assert_eq!(saved_b.purpose, SessionPurpose::Roadmap);
    assert_eq!(
        crate::application::saved_draft(&inputs, &a.conversation)
            .unwrap()
            .text,
        "A late draft"
    );
    assert_eq!(
        crate::application::saved_draft(&inputs, &b.conversation)
            .unwrap()
            .text,
        "B draft"
    );
}

async fn next(events: &mut UnboundedReceiver<Msg>) -> Msg {
    tokio::time::timeout(std::time::Duration::from_secs(5), events.recv())
        .await
        .unwrap()
        .unwrap()
}
async fn turn(events: &mut UnboundedReceiver<Msg>) {
    loop {
        match next(events).await {
            Msg::Agent(AgentEvent::TurnEnded(_)) => return,
            Msg::Agent(AgentEvent::Error(error) | AgentEvent::PromptRejected { error, .. }) => {
                panic!("{error}")
            }
            _ => {}
        }
    }
}

fn provider() -> (
    agent_client_protocol::Channel,
    tokio::task::JoinHandle<Result<(), agent_client_protocol::Error>>,
) {
    use agent_client_protocol::schema::ProtocolVersion;
    use agent_client_protocol::schema::v1::{
        AgentCapabilities, ContentBlock, ContentChunk, InitializeRequest, InitializeResponse,
        LoadSessionRequest, LoadSessionResponse, NewSessionRequest, NewSessionResponse,
        PromptRequest, PromptResponse, SessionNotification, SessionUpdate, StopReason, TextContent,
    };
    use agent_client_protocol::{Agent, Channel, on_receive_request};
    let (client, channel) = Channel::duplex();
    let task = tokio::spawn(async move {
        Agent
            .builder()
            .on_receive_request(
                async |_request: InitializeRequest, responder, _cx| {
                    responder.respond(
                        InitializeResponse::new(ProtocolVersion::V1)
                            .agent_capabilities(AgentCapabilities::new().load_session(true)),
                    )
                },
                on_receive_request!(),
            )
            .on_receive_request(
                async |_request: NewSessionRequest, responder, _cx| {
                    responder.respond(NewSessionResponse::new("provider"))
                },
                on_receive_request!(),
            )
            .on_receive_request(
                async |request: LoadSessionRequest, responder, cx| {
                    cx.send_notification(SessionNotification::new(
                        request.session_id,
                        SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(
                            TextContent::new("old replay"),
                        ))),
                    ))?;
                    responder.respond(LoadSessionResponse::new())
                },
                on_receive_request!(),
            )
            .on_receive_request(
                async |request: PromptRequest, responder, cx| {
                    cx.send_notification(SessionNotification::new(
                        request.session_id,
                        SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(
                            TextContent::new("new reply"),
                        ))),
                    ))?;
                    responder.respond(PromptResponse::new(StopReason::EndTurn))
                },
                on_receive_request!(),
            )
            .connect_to(channel)
            .await
    });
    (client, task)
}
