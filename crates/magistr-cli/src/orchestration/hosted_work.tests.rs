//! Starts a host cannot make are said in words, and the session goes on.
//! The started run itself — a judged iteration on the shared driver — is in
//! the loop driver's suite, beside the fake provider it needs.

use std::time::Duration;

use magistr_engine::NativeConfig;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use super::*;
use crate::error::MagistrError;

/// A project the workflow configuration loads, with `true` for every gate.
fn project() -> tempfile::TempDir {
    let dir = tempfile::TempDir::new().expect("a temp dir");
    let magistr = dir.path().join(".magistr");
    std::fs::create_dir_all(&magistr).expect("mkdir .magistr");
    let config = serde_json::json!({
        "project": {"name": "fixture", "description": "", "type": "cli"},
        "stack": {
            "language": "rust", "framework": "none", "package_manager": "cargo",
            "test_framework": "cargo-test", "linter": "clippy", "formatter": "rustfmt"
        },
        "quality_gate": {"check": "true", "test": "true", "lint": "true"},
        "format": {"command": "true", "extensions": ["rs"]},
        "spec": {"file": "ROADMAP.md", "unchecked_pattern": "^\\s*- \\[ \\]"},
        "testing": {"companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside"},
        "reference_repos": [],
        "state_prefix": "fixture"
    });
    std::fs::write(magistr.join("project.json"), config.to_string()).expect("project.json");
    std::fs::write(dir.path().join("ROADMAP.md"), "- [ ] make it work\n").expect("spec");
    dir
}

fn store(dir: &Path) -> NativeConfig {
    magistr_core::evidence::writer_config(dir, |_| None).with_data_dir(dir.join("tas-data"))
}

/// A connector no test here reaches: a refused start opens no session.
fn unreachable_connector() -> Connector {
    Connector::new(|_| async { Err(MagistrError::Config("no provider in this test".into())) })
}

struct Hosted {
    commands: UnboundedSender<Cmd>,
    events: UnboundedReceiver<Msg>,
    task: tokio::task::JoinHandle<Result<(), String>>,
}

fn hosted(dir: &Path, engines: EngineClient) -> Hosted {
    let inputs = crate::config::inputs::ProjectInputs::from_lookup(dir, |_| None);
    let config = AgentConfig::new("", dir).with_inputs(&inputs);
    let (inbox, events) = Inbox::channel();
    let (commands, receiver) = Commands::channel();
    let (_feed, tapped) = unbounded_channel();
    let session = interactive_session(
        config.clone(),
        unreachable_connector(),
        inbox.clone(),
        Some(engines.clone()),
    );
    let work = HostedWork::new(config, unreachable_connector(), inbox, engines, tapped);
    Hosted {
        commands,
        events,
        task: tokio::spawn(work.run(session, receiver)),
    }
}

async fn refusal(events: &mut UnboundedReceiver<Msg>, action: &LaunchAction) -> String {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match events.recv().await.expect("the inbox stays open") {
                Msg::CommandFailed {
                    cmd: Cmd::Launch(refused),
                    error,
                } if &refused == action => return error,
                _ => {}
            }
        }
    })
    .await
    .expect("the start is answered")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_host_that_only_attached_refuses_both_starts_in_words_and_stays_open() {
    let dir = project();
    let owner = EngineClient::with_config(store(dir.path())).expect("the first host");
    let endpoint = owner.serve_owner().await.expect("its endpoint");
    let attached = EngineClient::admit_or_attach_config(store(dir.path())).expect("attaches");
    assert!(attached.writer().is_none());

    let mut host = hosted(dir.path(), attached);
    for action in [
        LaunchAction::Loop(magistr_cockpit::cockpit::LoopScope::AllTracks),
        LaunchAction::Next,
    ] {
        host.commands.send(Cmd::Launch(action.clone())).unwrap();
        let error = refusal(&mut host.events, &action).await;
        assert!(
            error.contains("Another Magistr session owns this project's work"),
            "{error}"
        );
    }
    assert!(
        !dir.path().join(".magistr/loop-state.json").exists(),
        "a refused start persists no run"
    );
    drop(host.commands);
    host.task.await.unwrap().unwrap();
    drop(endpoint);
    drop(owner);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_held_loop_owner_lock_refuses_the_loop_in_words_and_stays_open() {
    let dir = project();
    let engines = EngineClient::with_config(store(dir.path())).expect("the host");
    let held = LoopLease::acquire(dir.path()).expect("another loop owns the project");

    let mut host = hosted(dir.path(), engines);
    for _ in 0..2 {
        host.commands
            .send(Cmd::Launch(LaunchAction::Loop(
                magistr_cockpit::cockpit::LoopScope::AllTracks,
            )))
            .unwrap();
        let error = refusal(
            &mut host.events,
            &LaunchAction::Loop(magistr_cockpit::cockpit::LoopScope::AllTracks),
        )
        .await;
        assert!(error.contains("A loop is already running"), "{error}");
    }
    assert!(!dir.path().join(".magistr/loop-state.json").exists());
    drop(held);
    drop(host.commands);
    host.task.await.unwrap().unwrap();
}
