use super::*;

use crate::roadmap::domain::ChunkStatus;

fn proposed(current: u32, suggested: u32) -> RoadmapEngine {
    let mut engine = RoadmapEngine::new("project".into());
    engine
        .add_chunk(
            "feature".into(),
            "Feature".into(),
            ChunkStatus::Pending,
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
        let chunk = engine.resolve_reprioritize("feature", accept).unwrap();
        assert_eq!(chunk.value.priority, expected);
        assert!(chunk.value.reprioritize.is_none());
        assert!(engine.resolve_reprioritize("feature", accept).is_err());
        assert_eq!(engine.roadmap().chunks[0].priority, expected);
    }
}

#[test]
fn status_resolution_rejects_without_a_racy_read_or_illegal_transition() {
    let mut engine = proposed(42, 1);
    engine
        .propose_status(
            "feature",
            ChunkStatus::Done,
            "Ahead".into(),
            "tracker".into(),
        )
        .unwrap();
    assert!(engine.resolve_status_proposal("feature", true).is_err());
    assert!(engine.roadmap().chunks[0].status_proposal.is_some());
    let chunk = engine.resolve_status_proposal("feature", false).unwrap();
    assert_eq!(chunk.value.status, ChunkStatus::Pending);
    assert!(chunk.value.status_proposal.is_none());
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
        crate::roadmap::broadcast::Broadcaster::from_engine(broadcaster.clone()),
    );
    engine.resolve_reprioritize("feature", true).unwrap();
    broadcaster
        .emit(
            crate::infra::Family::Roadmap,
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
    assert_eq!(changed["type"], "chunk_changed");
    assert_eq!(changed["chunk"]["priority"], u32::MAX);
    assert!(
        changed["chunk"]
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
