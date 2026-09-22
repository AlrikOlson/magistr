use super::*;

const T0: u64 = 1_755_750_000_000;

fn status() -> IndexStatus {
    IndexStatus {
        files: 120,
        parsed: 3,
        reused: 0,
        build_started_at_ms: Some(T0),
        build_updated_at_ms: Some(T0 + 500),
        ..IndexStatus::default()
    }
}

#[test]
fn no_build_kicked_means_no_task() {
    assert!(IndexBuildTask::derive(&IndexStatus::default()).is_none());
    let warm_without_build = IndexStatus {
        freshness: Freshness::Warm,
        generation: 3,
        ..IndexStatus::default()
    };
    assert!(IndexBuildTask::derive(&warm_without_build).is_none());
}

#[test]
fn building_is_working_with_progress_and_a_poll_interval() {
    let task = IndexBuildTask::derive(&IndexStatus {
        building: true,
        ..status()
    })
    .expect("a kicked build is a task");
    assert_eq!(task.state, IndexBuildState::Working);
    assert_eq!(task.message, "index build in progress: 3/120 files");
    assert_eq!(
        task.poll_interval_ms,
        Some(IndexBuildTask::POLL_INTERVAL_MS)
    );
    assert_eq!(
        (task.created_at_ms, task.last_updated_at_ms),
        (T0, T0 + 500)
    );
    assert!(!task.state.is_terminal());
}

#[test]
fn pending_refresh_is_working_even_between_builds() {
    let task = IndexBuildTask::derive(&IndexStatus {
        freshness: Freshness::Warming,
        refresh_pending: true,
        generation: 4,
        reused: 117,
        ..status()
    })
    .unwrap();
    assert_eq!(task.state, IndexBuildState::Working);
    assert_eq!(task.message, "index build in progress: 120/120 files");
    assert!(task.poll_interval_ms.is_some());
}

#[test]
fn a_failed_build_carries_the_error_verbatim() {
    let task = IndexBuildTask::derive(&IndexStatus {
        last_error: Some("scan blew up".into()),
        build_updated_at_ms: Some(T0 + 900),
        ..status()
    })
    .expect("task");
    assert_eq!(task.state, IndexBuildState::Failed);
    assert_eq!(task.message, "index build failed: scan blew up");
    assert_eq!(task.poll_interval_ms, None);
    assert!(task.state.is_terminal());
}

#[test]
fn a_warm_index_after_a_build_is_completed_with_generation_and_files() {
    let task = IndexBuildTask::derive(&IndexStatus {
        freshness: Freshness::Warm,
        generation: 4,
        parsed: 120,
        ..status()
    })
    .expect("task");
    assert_eq!(task.state, IndexBuildState::Completed);
    assert_eq!(task.message, "index at tier 1, generation 4 (120 files)");
    assert_eq!(task.state.as_str(), "completed");
}

#[test]
fn before_the_coordinator_stamps_a_start_the_task_is_working_since_open() {
    let fresh = IndexStatus::default();
    let task = IndexBuildTask::since_open(&fresh, T0).expect("kicked at open");
    assert_eq!(task.state, IndexBuildState::Working);
    assert_eq!((task.created_at_ms, task.last_updated_at_ms), (T0, T0));
    let failed_open = IndexStatus {
        last_error: Some("cache locked".into()),
        ..IndexStatus::default()
    };
    let task = IndexBuildTask::since_open(&failed_open, T0).expect("task");
    assert_eq!(task.state, IndexBuildState::Failed);
    let stamped = IndexBuildTask::since_open(&status(), T0 + 9).expect("task");
    assert_eq!(
        stamped.created_at_ms, T0,
        "the coordinator's own stamp wins"
    );
}

#[test]
fn the_settle_race_stays_working_and_falls_back_to_the_start_time() {
    let task = IndexBuildTask::derive(&IndexStatus {
        parsed: 120,
        build_updated_at_ms: None,
        ..status()
    })
    .expect("task");
    assert_eq!(task.state, IndexBuildState::Working);
    assert_eq!(task.last_updated_at_ms, T0);
}
