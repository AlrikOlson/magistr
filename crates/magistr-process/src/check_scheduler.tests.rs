use super::*;
#[tokio::test]
async fn resources_serialize_cargo_but_independent_work_overlaps_and_cancel_releases_queue() {
    let dir = tempfile::tempdir().unwrap();
    let cancel = CancellationToken::new();
    let cargo = acquire_check(
        dir.path(),
        "cargo clippy",
        &CheckSchedule::default(),
        Duration::from_secs(2),
        &cancel,
    )
    .await
    .unwrap();
    let independent = CheckSchedule {
        max_parallel: 3,
        resources: Some(vec![]),
    };
    let declared = CheckSchedule {
        max_parallel: 4,
        resources: Some(vec!["cargo".into()]),
    };
    assert!(matches!(
        acquire_check(
            dir.path(),
            "./rust-check.sh",
            &declared,
            Duration::from_millis(10),
            &cancel
        )
        .await,
        Err(CheckWaitError::TimedOut)
    ));
    let other = acquire_check(
        dir.path(),
        "lint",
        &independent,
        Duration::from_secs(1),
        &cancel,
    )
    .await
    .unwrap();
    assert!(matches!(
        acquire_check(
            dir.path(),
            "cargo test",
            &CheckSchedule::default(),
            Duration::from_millis(10),
            &cancel
        )
        .await,
        Err(CheckWaitError::TimedOut)
    ));
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(matches!(
        acquire_check(
            dir.path(),
            "script",
            &CheckSchedule::default(),
            Duration::from_secs(1),
            &cancelled
        )
        .await,
        Err(CheckWaitError::Cancelled)
    ));
    drop(cargo);
    drop(other);
    acquire_check(
        dir.path(),
        "cargo test",
        &CheckSchedule::default(),
        Duration::from_secs(1),
        &cancel,
    )
    .await
    .unwrap();
}
#[tokio::test]
async fn capacity_and_opaque_commands_are_enforced() {
    let dir = tempfile::tempdir().unwrap();
    let cancel = CancellationToken::new();
    let policy = CheckSchedule {
        max_parallel: 1,
        resources: Some(vec![]),
    };
    let first = acquire_check(dir.path(), "a", &policy, Duration::from_secs(1), &cancel)
        .await
        .unwrap();
    assert!(matches!(
        acquire_check(dir.path(), "b", &policy, Duration::from_millis(10), &cancel).await,
        Err(CheckWaitError::TimedOut)
    ));
    assert!(matches!(
        acquire_check(
            dir.path(),
            "./unknown.sh",
            &CheckSchedule::default(),
            Duration::from_millis(10),
            &cancel
        )
        .await,
        Err(CheckWaitError::TimedOut)
    ));
    drop(first);
    acquire_check(
        dir.path(),
        "./unknown.sh",
        &CheckSchedule::default(),
        Duration::from_secs(1),
        &cancel,
    )
    .await
    .unwrap();
}
#[tokio::test]
async fn aborting_a_waiter_does_not_leave_an_exclusive_request_behind() {
    let dir = tempfile::tempdir().unwrap();
    let cancel = CancellationToken::new();
    let first = acquire_check(
        dir.path(),
        "cargo test",
        &CheckSchedule::default(),
        Duration::from_secs(1),
        &cancel,
    )
    .await
    .unwrap();
    let root = dir.path().to_owned();
    let job = tokio::spawn(async move {
        acquire_check(
            &root,
            "./opaque.sh",
            &CheckSchedule::default(),
            Duration::from_secs(5),
            &CancellationToken::new(),
        )
        .await
    });
    tokio::task::yield_now().await;
    job.abort();
    let _ = job.await;
    drop(first);
    acquire_check(
        dir.path(),
        "cargo test",
        &CheckSchedule::default(),
        Duration::from_secs(1),
        &cancel,
    )
    .await
    .unwrap();
}
