use super::*;
use magistr_cockpit::cockpit::{EngineCmd, WorkbenchCmd};

#[tokio::test]
async fn closed_local_and_provider_receivers_preserve_the_refused_action() {
    let dir = tempfile::tempdir().unwrap();
    let (inbox, mut events) = Inbox::channel();
    let mut session = InteractiveSession::new(
        AgentConfig::new("", dir.path()),
        Connector::new(|_| async { panic!("forwarding must not start a provider") }),
        inbox,
        None,
    );
    session.local_task.abort();
    let _ = (&mut session.local_task).await;
    let (provider, receiver) = Commands::channel();
    drop(receiver);
    let (_, provider_events) = Inbox::channel();
    session.connection = Some(Connection {
        task: tokio::spawn(async { Ok(()) }),
        commands: Some(provider),
        events: provider_events,
        roadmap: false,
    });
    for cmd in [
        Cmd::Prompt("Keep @src/main.rs".into()),
        Cmd::Cancel,
        Cmd::Permission {
            id: 5,
            option_id: "allow".into(),
        },
        Cmd::Workbench(WorkbenchCmd::Undo { id: "op-2".into() }),
        Cmd::Engine(EngineCmd::Checkpoint),
    ] {
        assert!(!session.forward(cmd.clone()));
        assert!(matches!(events.recv().await,
            Some(Msg::CommandFailed { cmd: refused, error }) if refused == cmd && error.contains("connection closed")));
        assert!(
            events.try_recv().is_err(),
            "one outcome for one failed action"
        );
    }
    session.close().await;
}

#[tokio::test]
async fn delivery_to_an_open_receiver_does_not_emit_a_refusal() {
    let dir = tempfile::tempdir().unwrap();
    let (inbox, mut events) = Inbox::channel();
    let mut session = InteractiveSession::new(
        AgentConfig::new("", dir.path()),
        Connector::new(|_| async { panic!("forwarding must not start a provider") }),
        inbox,
        None,
    );
    let (provider, mut receiver) = Commands::channel();
    let (_, provider_events) = Inbox::channel();
    session.connection = Some(Connection {
        task: tokio::spawn(async { Ok(()) }),
        commands: Some(provider),
        events: provider_events,
        roadmap: false,
    });
    assert!(session.forward(Cmd::Cancel));
    assert_eq!(receiver.next().await, Some(Cmd::Cancel));
    assert!(events.try_recv().is_err());
    session.close().await;
    session.local_task.abort();
    let _ = (&mut session.local_task).await;
}

#[tokio::test]
async fn unsupported_controls_and_absent_provider_fail_without_ending_live_work() {
    let (inbox, mut messages) = Inbox::channel();
    let (commands_tx, commands) = Commands::channel();
    let task = spawn_session(
        Box::new(crate::orchestration::local_session::LocalSession),
        inbox,
        commands,
        None,
        None,
        None,
    );
    let mut model =
        magistr_cockpit::cockpit::Model::new(magistr_cockpit::cockpit::Size::new(80, 24));
    magistr_cockpit::cockpit::update(
        &mut model,
        Msg::Agent(AgentEvent::Text("Still working".into())),
    );
    for cmd in [
        Cmd::Cancel,
        Cmd::Permission {
            id: 17,
            option_id: "allow".into(),
        },
        Cmd::Constraint {
            id: 17,
            condition: "read only".into(),
        },
        Cmd::Delegate("read".into()),
        Cmd::Kill(magistr_cockpit::cockpit::KillCmd::Terminal("build".into())),
        Cmd::Kill(magistr_cockpit::cockpit::KillCmd::Gate("checks".into())),
        Cmd::Reconnect,
        Cmd::Loop(magistr_cockpit::cockpit::LoopCmd::Pause),
    ] {
        commands_tx.send(cmd.clone()).unwrap();
        let event = tokio::time::timeout(Duration::from_secs(2), messages.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(
            matches!(&event, Msg::CommandFailed { cmd: refused, error } if refused == &cmd && !error.is_empty())
        );
        magistr_cockpit::cockpit::update(&mut model, event);
        assert!(
            model.agent_activity().active,
            "an action refusal cannot finish unrelated work"
        );
    }
    drop(commands_tx);
    task.await.unwrap().unwrap();
}
