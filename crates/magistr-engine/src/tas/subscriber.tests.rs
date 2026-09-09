use std::time::Duration;

use think_and_ship::infra::{Broadcaster, Family};

use super::*;
use crate::cost::BudgetConfig;
use crate::handle::EngineEvent;
use crate::workflow::WorkflowConfig;

fn handle() -> EngineHandle {
    EngineHandle::new(
        "sub-test",
        WorkflowConfig {
            project_dir: ".".into(),
            spec_path: String::new(),
            quality_gate: None,
            max_iterations: 1,
            lifecycle_hooks: vec![],
        },
        BudgetConfig::none(),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn frames_on_the_socket_become_native_events() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let sock = tmp.path().join("b.sock");
    let broadcaster = Broadcaster::spawn(sock.clone()).expect("bind broadcast socket");

    let engine = handle();
    let mut rx = engine.subscribe();
    let task = spawn_broadcast_subscriber(sock, engine.clone());
    assert!(
        broadcaster
            .wait_for_subscribers(1, Duration::from_secs(5))
            .await,
        "subscriber registered"
    );

    broadcaster
        .emit(
            Family::Roadmap,
            &serde_json::json!({"type": "chunk_changed", "chunk": {"id": "a"}}),
        )
        .expect("emit");

    let event = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("event within timeout")
        .expect("channel open");
    match event {
        EngineEvent::Native(frame) => {
            assert_eq!(frame.family, "roadmap");
            assert_eq!(frame.kind, "chunk_changed");
            assert_eq!(frame.payload["chunk"]["id"], "a");
        }
        other => panic!("expected a native frame, got {other:?}"),
    }
    task.abort();
}

#[tokio::test]
async fn missing_socket_ends_quietly() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let engine = handle();
    let task = spawn_broadcast_subscriber(tmp.path().join("nope.sock"), engine);
    tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .expect("task finished")
        .expect("task did not panic");
}
