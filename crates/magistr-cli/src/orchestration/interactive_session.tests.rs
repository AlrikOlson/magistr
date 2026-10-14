use super::*;
use magistr_application::sessions::AgentEvent;
use magistr_cockpit::cockpit::{Cmd, Msg};

async fn next_message(events: &mut tokio::sync::mpsc::UnboundedReceiver<Msg>) -> Msg {
    tokio::time::timeout(std::time::Duration::from_secs(3), events.recv())
        .await
        .expect("session progress")
        .expect("session observer")
}

async fn turn_text(events: &mut tokio::sync::mpsc::UnboundedReceiver<Msg>) -> Vec<String> {
    let mut text = Vec::new();
    loop {
        match next_message(events).await {
            Msg::Agent(AgentEvent::Text(value)) => text.push(value),
            Msg::Agent(AgentEvent::TurnEnded(_)) => return text,
            Msg::Agent(AgentEvent::Error(error) | AgentEvent::PromptRejected { error, .. }) => {
                panic!("turn failed: {error}")
            }
            _ => {}
        }
    }
}

#[tokio::test]
async fn host_reconnect_loads_the_same_session_without_repeating_history() {
    for planning in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let config = AgentConfig::new("", dir.path());
        let (observed, mut opened) = tokio::sync::mpsc::unbounded_channel();
        let connector = Connector::new(move |config| {
            let observed = observed.clone();
            async move {
                let spec = crate::agent::ResolvedSessionSpec::resolve_with_runtime(
                    config.clone(),
                    &crate::config::user_config::UserConfig::default(),
                    crate::agent::AgentRuntime {
                        path: "controlled".into(),
                        args: vec![],
                        auth: crate::agent::AuthMethod::Implicit,
                        provider: "scripted".into(),
                    },
                )?;
                let history_inputs =
                    crate::config::inputs::ProjectInputs::from_lookup(&config.cwd, |key| {
                        (key == "XDG_DATA_HOME")
                            .then(|| config.cwd.join("session-data").display().to_string())
                    });
                spec.history()
                    .begin(&history_inputs, &config.cwd, "scripted", spec.model())
                    .await?;
                let (channel, agent) = replaying_agent();
                let mut transport =
                    magistr_acp::AcpTransport::new_with_args("controlled".into(), vec![], None)
                        .unwrap()
                        .session_mode(spec.session_mode());
                transport
                    .connect_over(channel)
                    .await
                    .map_err(|error| crate::error::MagistrError::Acp(error.to_string()))?;
                observed.send((config, agent)).unwrap();
                Ok((transport, spec))
            }
        });
        let (inbox, mut events) = Inbox::channel();
        let (commands, receiver) = magistr_cockpit_runtime::channel::Commands::channel();
        let task = interactive_session(config, connector, inbox, None).spawn(receiver);
        let prompt = |text: &str| {
            if planning {
                Cmd::RoadmapPrompt(text.into())
            } else {
                Cmd::Prompt(text.into())
            }
        };
        commands.send(prompt("First request")).unwrap();
        let (initial, agent) = opened.recv().await.unwrap();
        assert!(initial.resume.is_none());
        assert_eq!(turn_text(&mut events).await, ["New reply"]);
        agent.abort();
        let _ = agent.await;
        loop {
            if matches!(
                next_message(&mut events).await,
                Msg::Agent(AgentEvent::Error(_))
            ) {
                break;
            }
        }
        commands.send(prompt("Continue")).unwrap();
        let (restored, agent) = opened.recv().await.unwrap();
        assert_eq!(restored.resume.as_deref(), Some("retained-session"));
        assert_eq!(
            restored.history.purpose() == SessionPurpose::Roadmap,
            planning
        );
        assert!(!restored.continue_conversation && restored.prompt.is_empty());
        assert_eq!(
            turn_text(&mut events).await,
            ["New reply"],
            "session/load history must not become the next request's reply"
        );
        drop(commands);
        task.await.unwrap().unwrap();
        agent.abort();
        let _ = agent.await;
    }
}

fn replaying_agent() -> (
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
    let (client, agent) = Channel::duplex();
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
                    responder.respond(NewSessionResponse::new("retained-session"))
                },
                on_receive_request!(),
            )
            .on_receive_request(
                async |request: LoadSessionRequest, responder, cx| {
                    assert_eq!(request.session_id.to_string(), "retained-session");
                    cx.send_notification(SessionNotification::new(
                        request.session_id,
                        SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(
                            TextContent::new("Old reply"),
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
                            TextContent::new("New reply"),
                        ))),
                    ))?;
                    responder.respond(PromptResponse::new(StopReason::EndTurn))
                },
                on_receive_request!(),
            )
            .connect_to(agent)
            .await
    });
    (client, task)
}

#[tokio::test]
async fn host_connector_preserves_choices_and_separates_planning_configuration() {
    for (command, planning) in [
        (Cmd::Prompt("Continue".into()), false),
        (Cmd::RoadmapPrompt("Plan".into()), true),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let mut config = AgentConfig::new("old opening", dir.path()).model("chosen");
        config.resume = Some("old-session".into());
        config.continue_conversation = true;
        config.workflow_permissions = true;
        let (observed, mut received) = tokio::sync::mpsc::unbounded_channel();
        let connector = Connector::new(move |config| {
            observed.send(config).unwrap();
            async { Err(crate::error::MagistrError::Acp("connection refused".into())) }
        });
        let (inbox, mut events) = Inbox::channel();
        let (commands, receiver) = magistr_cockpit_runtime::channel::Commands::channel();
        let task = interactive_session(config, connector, inbox, None).spawn(receiver);
        commands.send(command).unwrap();
        let actual = tokio::time::timeout(std::time::Duration::from_secs(2), received.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(actual.model.as_deref(), Some("chosen"));
        assert!(
            actual.prompt.is_empty() && actual.resume.is_none() && !actual.continue_conversation
        );
        assert_eq!(
            actual.history.purpose() == SessionPurpose::Roadmap,
            planning
        );
        assert!(actual.workflow_permissions);
        assert!(
            matches!(events.recv().await, Some(Msg::Agent(AgentEvent::PromptRejected { error, .. })) if error.contains("connection refused"))
        );
        drop(commands);
        task.await.unwrap().unwrap();
    }
}
