//! Completion announces the same step that the owning engine saved.
#![cfg(unix)]
use magistr_workflow::infra::Broadcaster;
use magistr_workflow::run::broadcast::Broadcaster as RunBroadcaster;
use magistr_workflow::run::domain::artifact::{Artifact, ArtifactType};
use magistr_workflow::run::domain::step::StepType;
use magistr_workflow::run::engine::RunEngine;
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
        RunEngine::new("test".into()).with_broadcaster(RunBroadcaster::from_engine(broadcaster));
    engine
        .set_goal(
            "Read the completed record".into(),
            vec![],
            vec![],
            String::new(),
        )
        .unwrap();
    engine
        .add_step(
            "step".into(),
            "Save results".into(),
            StepType::Implement,
            None,
            None,
        )
        .unwrap();
    engine.start_step("step").unwrap();
    let step = engine
        .complete_step(
            "step",
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
        if value["type"] == "step_done" {
            assert_eq!(value["step"], serde_json::to_value(&step).unwrap());
            assert!(step.completed_at.is_some());
            assert_eq!(step.artifacts.len(), 1);
            return;
        }
    }
    panic!("completion event was not delivered");
}
