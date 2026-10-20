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
    magistr_core::evidence::writer_config(dir, |_| None).with_data_dir(dir.join("workflow-state"))
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
        assert!(error.contains("Work is already running"), "{error}");
    }
    assert!(!dir.path().join(".magistr/loop-state.json").exists());
    drop(held);
    drop(host.commands);
    host.task.await.unwrap().unwrap();
}

#[test]
fn both_admitted_modes_exclude_every_start_before_checkpoint_writes() {
    use magistr_cockpit::cockpit::LoopScope;
    for first in [LaunchAction::Next, LaunchAction::Loop(LoopScope::AllTracks)] {
        let dir = project();
        let engines = EngineClient::with_config(store(dir.path())).unwrap();
        let admitted = prepare(dir.path(), &engines, &first, None).unwrap();
        let saved = std::fs::read(dir.path().join(".magistr/loop-state.json")).unwrap();
        for other in [LaunchAction::Next, LaunchAction::Loop(LoopScope::AllTracks)] {
            let error = prepare(dir.path(), &engines, &other, None)
                .err()
                .expect("lease refusal");
            assert!(error.contains("already running"), "{error}");
            assert_eq!(
                std::fs::read(dir.path().join(".magistr/loop-state.json")).unwrap(),
                saved
            );
        }
        drop(admitted);
        assert!(LoopLease::acquire(dir.path()).is_ok());
    }
}

#[test]
fn one_item_refuses_unfinished_loop_without_erasing_its_progress() {
    let dir = project();
    let engines = EngineClient::with_config(store(dir.path())).unwrap();
    let loop_run = prepare(
        dir.path(),
        &engines,
        &LaunchAction::Loop(magistr_cockpit::cockpit::LoopScope::AllTracks),
        None,
    )
    .unwrap();
    let path = dir.path().join(".magistr/loop-state.json");
    let saved = std::fs::read(&path).unwrap();
    drop(loop_run);
    let error = prepare(dir.path(), &engines, &LaunchAction::Next, None)
        .err()
        .expect("mode mismatch");
    assert!(error.contains("unfinished"), "{error}");
    assert_eq!(std::fs::read(path).unwrap(), saved);
}

#[test]
fn admitted_work_freezes_its_agent_and_refuses_recovery_with_a_different_selection() {
    use crate::config::session_selection::SessionSelection;
    let dir = project();
    let engines = EngineClient::with_config(store(dir.path())).unwrap();
    let chosen = SessionSelection {
        agent: "copilot".into(),
        model: Some("chosen-model".into()),
        effort: None,
        sources: Default::default(),
    };
    let prepared = prepare(dir.path(), &engines, &LaunchAction::Next, Some(&chosen)).unwrap();
    let state_path = dir.path().join(".magistr/loop-state.json");
    let saved = std::fs::read(&state_path).unwrap();
    let state: crate::workflow::iteration::IterationState = serde_json::from_slice(&saved).unwrap();
    assert_eq!(state.session_selection.as_ref(), Some(&chosen));
    drop(prepared);
    let other = SessionSelection {
        agent: "claude".into(),
        ..chosen
    };
    assert!(prepare(dir.path(), &engines, &LaunchAction::Next, Some(&other)).is_err());
    assert_eq!(std::fs::read(state_path).unwrap(), saved);
}
