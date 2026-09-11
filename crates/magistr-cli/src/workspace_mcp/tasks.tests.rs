use super::*;

fn working() -> IndexBuildTask {
    IndexBuildTask {
        state: IndexBuildState::Working,
        message: "index build in progress: 3/120 files".into(),
        created_at_ms: 1_755_750_000_000,
        last_updated_at_ms: 1_755_750_000_500,
        poll_interval_ms: Some(200),
    }
}

#[test]
fn the_listing_row_is_the_retained_shape() {
    let row = listing(&working());
    assert_eq!(
        row,
        json!({
            "taskId": "index/build",
            "status": "working",
            "statusMessage": "index build in progress: 3/120 files",
            "createdAt": "2025-08-21T04:20:00.000Z",
            "lastUpdatedAt": "2025-08-21T04:20:00.500Z",
            "ttl": null,
            "pollInterval": 200
        })
    );
    let done = listing(&IndexBuildTask {
        state: IndexBuildState::Completed,
        message: "index at tier 1, generation 4 (120 files)".into(),
        poll_interval_ms: None,
        ..working()
    });
    assert_eq!(done["status"], "completed");
    assert!(done.get("pollInterval").is_none());
}

#[test]
fn the_detailed_task_carries_status_message_and_poll_interval() {
    let task = detailed(&working());
    assert_eq!(task.status(), TaskStatus::Working);
    let wire = serde_json::to_value(&task).expect("json");
    assert_eq!(wire["taskId"], "index/build");
    assert_eq!(wire["status"], "working");
    assert_eq!(
        wire["statusMessage"],
        "index build in progress: 3/120 files"
    );
    assert_eq!(wire["pollIntervalMs"], 200);
    let failed = detailed(&IndexBuildTask {
        state: IndexBuildState::Failed,
        message: "index build failed: scan blew up".into(),
        poll_interval_ms: None,
        ..working()
    });
    assert_eq!(failed.status(), TaskStatus::Failed);
    let wire = serde_json::to_value(&failed).expect("json");
    assert_eq!(wire["error"]["message"], "index build failed: scan blew up");
}
