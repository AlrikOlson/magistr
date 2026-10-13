use super::super::session_registry::tests::{Log, projects, registry};
use super::*;
use magistr_application::sessions::AgentEvent;
use tokio::sync::mpsc;

async fn next(events: &mut mpsc::UnboundedReceiver<Msg>) -> Msg {
    tokio::time::timeout(Duration::from_secs(3), events.recv())
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn switching_views_retains_hosts_and_replays_unseen_observations() {
    let tmp = projects();
    let log = Log::default();
    let (inbox, mut events) = Inbox::channel();
    let (commands, receiver) = Commands::channel();
    let (window, first) = ProjectWindow::start_with(
        registry(log.clone()),
        tmp.path().join("first"),
        inbox,
        receiver,
    )
    .await
    .unwrap();
    let address = first.session_address.unwrap();
    commands
        .send(Cmd::Session {
            address: address.clone(),
            command: Box::new(Cmd::OpenProject(
                tmp.path().join("second").display().to_string(),
            )),
            draft: String::new(),
        })
        .unwrap();
    let Msg::SessionFrame {
        address: second,
        seed: Some(_),
        ..
    } = next(&mut events).await
    else {
        panic!("selected second");
    };
    assert_eq!(log.lines(), ["open first", "open second"]);
    commands
        .send(Cmd::Session {
            address: address.clone(),
            command: Box::new(Cmd::Prompt("hidden stream".into())),
            draft: "first draft".into(),
        })
        .unwrap();
    commands
        .send(Cmd::Session {
            address: second,
            command: Box::new(Cmd::OpenProject(
                tmp.path().join("first").display().to_string(),
            )),
            draft: "second draft".into(),
        })
        .unwrap();
    loop {
        let message = next(&mut events).await;
        if let Msg::SessionFrame {
            address: selected,
            seed: Some(seed),
            ..
        } = message
        {
            assert_eq!(selected, address);
            assert_eq!(seed.draft.as_deref(), Some("first draft"));
            // The prompt can complete before or after selection; both deliveries
            // name first and cannot land in second.
            let mut replay = seed.replay;
            if !replay.iter().any(|msg| matches!(msg, Msg::Agent(AgentEvent::Text(text)) if text == "first: hidden stream")) {
                let Msg::SessionFrame { address: update, messages, .. } = next(&mut events).await else { panic!("first stream"); };
                assert_eq!(update, address); replay.extend(messages);
            }
            assert!(replay.iter().any(|msg| matches!(msg, Msg::Agent(AgentEvent::Text(text)) if text == "first: hidden stream")));
            break;
        }
    }
    window.shutdown().await.unwrap();
    assert_eq!(
        log.lines()
            .iter()
            .filter(|line| line.starts_with("settled"))
            .count(),
        2
    );
}

#[tokio::test]
async fn provider_failure_keeps_the_selected_host_and_draft_available() {
    let tmp = projects();
    let log = Log::default();
    let (inbox, mut events) = Inbox::channel();
    let (commands, receiver) = Commands::channel();
    let (window, first) = ProjectWindow::start_with(
        registry(log.clone()),
        tmp.path().join("first"),
        inbox,
        receiver,
    )
    .await
    .unwrap();
    commands
        .send(Cmd::Session {
            address: first.session_address.clone().unwrap(),
            command: Box::new(Cmd::OpenProject(
                tmp.path().join("held").display().to_string(),
            )),
            draft: "keep".into(),
        })
        .unwrap();
    assert!(
        matches!(next(&mut events).await, Msg::CommandFailed { error, .. } if error.contains("provider unavailable"))
    );
    commands
        .send(Cmd::Session {
            address: first.session_address.unwrap(),
            command: Box::new(Cmd::Prompt("still here".into())),
            draft: "keep".into(),
        })
        .unwrap();
    assert!(
        matches!(next(&mut events).await, Msg::SessionFrame { messages, .. } if messages.iter().any(|msg| matches!(msg, Msg::Agent(AgentEvent::Text(text)) if text == "first: still here")))
    );
    window.shutdown().await.unwrap();
}

#[tokio::test]
async fn a_closed_selected_session_does_not_block_navigation_or_stop_other_sessions() {
    let tmp = projects();
    let log = Log::default();
    let registry = registry(log.clone());
    let client = registry.client();
    let (inbox, mut events) = Inbox::channel();
    let (commands, receiver) = Commands::channel();
    let (window, first) =
        ProjectWindow::start_with(registry, tmp.path().join("first"), inbox, receiver)
            .await
            .unwrap();
    let first = first.session_address.unwrap();
    let second = client.open(tmp.path().join("second")).await.unwrap();
    client.close(&first).await.unwrap();
    commands
        .send(Cmd::Session {
            address: first,
            command: Box::new(Cmd::OpenProject(
                tmp.path().join("second").display().to_string(),
            )),
            draft: String::new(),
        })
        .unwrap();
    loop {
        match next(&mut events).await {
            Msg::SessionFrame {
                address,
                seed: Some(_),
                ..
            } => {
                assert_eq!(address, second);
                break;
            }
            Msg::CommandFailed { error, .. } => panic!("navigation refused: {error}"),
            _ => {}
        }
    }
    assert_eq!(client.list().await.unwrap().len(), 1);
    assert!(!log.lines().iter().any(|line| line == "settled second"));
    window.shutdown().await.unwrap();
}
