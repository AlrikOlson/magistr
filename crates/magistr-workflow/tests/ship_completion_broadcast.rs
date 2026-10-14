//! Completion announces the same task that the owning engine saved.
#![cfg(unix)]
use magistr_workflow::infra::Broadcaster;
use magistr_workflow::ship::broadcast::Broadcaster as ShipBroadcaster;
use magistr_workflow::ship::domain::artifact::{Artifact, ArtifactType};
use magistr_workflow::ship::domain::task::TaskType;
use magistr_workflow::ship::engine::ShipEngine;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};

#[tokio::test]
async fn completion_broadcast_keeps_artifacts_and_completion_time() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("completion.sock");
    let broadcaster = Broadcaster::spawn(path.clone()).unwrap();
    let stream = tokio::net::UnixStream::connect(path).await.unwrap();
    assert!(
        broadcaster
            .wait_for_subscribers(1, Duration::from_secs(5))
            .await
    );
    let mut engine =
        ShipEngine::new("test".into()).with_broadcaster(ShipBroadcaster::from_engine(broadcaster));
    engine
        .set_objective(
            "Read the completed record".into(),
            vec![],
            vec![],
            String::new(),
        )
        .unwrap();
    engine
        .add_task(
            "task".into(),
            "Save results".into(),
            TaskType::Implement,
            None,
            None,
        )
        .unwrap();
    engine.start_task("task").unwrap();
    let task = engine
        .complete_task(
            "task",
            vec![Artifact {
                artifact_type: ArtifactType::File,
                reference: "report.txt".into(),
                description: "Saved results".into(),
            }],
        )
        .unwrap()
        .value
        .clone();
    let mut lines = BufReader::new(stream).lines();
    for _ in 0..4 {
        let line = tokio::time::timeout(Duration::from_secs(5), lines.next_line())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&line).unwrap();
        if value["type"] == "task_completed" {
            assert_eq!(value["task"], serde_json::to_value(&task).unwrap());
            assert!(task.completed_at.is_some());
            assert_eq!(task.artifacts.len(), 1);
            return;
        }
    }
    panic!("completion event was not delivered");
}
