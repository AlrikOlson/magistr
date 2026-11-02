use super::*;

use crate::plan::domain::TaskStatus;

fn proposed(current: u32, suggested: u32) -> PlanEngine {
    let mut engine = PlanEngine::new("project".into());
    engine
        .add_task(
            "feature".into(),
            "Feature".into(),
            TaskStatus::ToDo,
            current,
            String::new(),
            vec![],
            vec![],
            false,
        )
        .unwrap();
    engine
        .propose_reprioritize("feature", suggested, "Human decides".into())
        .unwrap();
    engine
}

#[test]
fn priority_resolution_handles_rejection_same_value_and_integer_boundary() {
    for (current, suggested, accept, expected) in [
        (42, 42, true, 42),
        (42, 1, false, 42),
        (42, 1, true, 1),
        (u32::MAX, u32::MAX, true, u32::MAX),
        (0, 0, false, 0),
    ] {
        let mut engine = proposed(current, suggested);
        let task = engine.resolve_reprioritize("feature", accept).unwrap();
        assert_eq!(task.value.priority, expected);
        assert!(task.value.reprioritize.is_none());
        assert!(engine.resolve_reprioritize("feature", accept).is_err());
        assert_eq!(engine.plan().tasks[0].priority, expected);
    }
}

#[test]
fn status_resolution_rejects_without_a_racy_read_or_illegal_transition() {
    let mut engine = proposed(42, 1);
    engine
        .propose_status(
            "feature",
            TaskStatus::Done,
            "Ahead".into(),
            "tracker".into(),
        )
        .unwrap();
    assert!(engine.resolve_status_proposal("feature", true).is_err());
    assert!(engine.plan().tasks[0].status_proposal.is_some());
    let task = engine.resolve_status_proposal("feature", false).unwrap();
    assert_eq!(task.value.status, TaskStatus::ToDo);
    assert!(task.value.status_proposal.is_none());
}

#[cfg(unix)]
#[tokio::test]
async fn same_priority_resolution_publishes_one_final_change_without_detour() {
    use std::time::Duration;
    use tokio::io::{AsyncBufReadExt, BufReader};
    let directory = tempfile::tempdir().unwrap();
    let socket = directory.path().join("resolution.sock");
    let broadcaster = crate::infra::Broadcaster::spawn(socket.clone()).unwrap();
    let stream = tokio::net::UnixStream::connect(socket).await.unwrap();
    assert!(
        broadcaster
            .wait_for_subscribers(1, Duration::from_secs(10))
            .await
    );
    let mut engine = proposed(u32::MAX, u32::MAX).with_broadcaster(
        crate::plan::broadcast::Broadcaster::from_engine(broadcaster.clone()),
    );
    engine.resolve_reprioritize("feature", true).unwrap();
    broadcaster
        .emit(
            crate::infra::Family::Plan,
            &serde_json::json!({"type": "barrier"}),
        )
        .unwrap();
    let mut lines = BufReader::new(stream).lines();
    let first = tokio::time::timeout(Duration::from_secs(10), lines.next_line())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let changed: serde_json::Value = serde_json::from_str(&first).unwrap();
    assert_eq!(changed["type"], "task_changed");
    assert_eq!(changed["task"]["priority"], u32::MAX);
    assert!(
        changed["task"]
            .get("reprioritize")
            .is_none_or(serde_json::Value::is_null)
    );
    let second = tokio::time::timeout(Duration::from_secs(10), lines.next_line())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let barrier: serde_json::Value = serde_json::from_str(&second).unwrap();
    assert_eq!(
        barrier["type"], "barrier",
        "no intermediate priority can be published"
    );
}
