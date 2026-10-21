use super::*;
#[tokio::test]
async fn resources_serialize_cargo_but_independent_work_overlaps_and_cancel_releases_queue() {
    let dir = tempfile::tempdir().unwrap();
    let cancel = CancellationToken::new();
    let cargo = acquire_gate(
        dir.path(),
        "cargo clippy",
        &GateSchedule::default(),
        Duration::from_secs(2),
        &cancel,
    )
    .await
    .unwrap();
    let independent = GateSchedule {
        max_parallel: 3,
        resources: Some(vec![]),
    };
    let declared = GateSchedule {
        max_parallel: 4,
        resources: Some(vec!["cargo".into()]),
    };
    assert!(matches!(
        acquire_gate(
            dir.path(),
            "./rust-gate.sh",
            &declared,
            Duration::from_millis(10),
            &cancel
        )
        .await,
        Err(GateWaitError::TimedOut)
    ));
    let other = acquire_gate(
        dir.path(),
        "lint",
        &independent,
        Duration::from_secs(1),
        &cancel,
    )
    .await
    .unwrap();
    assert!(matches!(
        acquire_gate(
            dir.path(),
            "cargo test",
            &GateSchedule::default(),
            Duration::from_millis(10),
            &cancel
        )
        .await,
        Err(GateWaitError::TimedOut)
    ));
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(matches!(
        acquire_gate(
            dir.path(),
            "script",
            &GateSchedule::default(),
            Duration::from_secs(1),
            &cancelled
        )
        .await,
        Err(GateWaitError::Cancelled)
    ));
    drop(cargo);
    drop(other);
    acquire_gate(
        dir.path(),
        "cargo test",
        &GateSchedule::default(),
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
    let policy = GateSchedule {
        max_parallel: 1,
        resources: Some(vec![]),
    };
    let first = acquire_gate(dir.path(), "a", &policy, Duration::from_secs(1), &cancel)
        .await
        .unwrap();
    assert!(matches!(
        acquire_gate(dir.path(), "b", &policy, Duration::from_millis(10), &cancel).await,
        Err(GateWaitError::TimedOut)
    ));
    assert!(matches!(
        acquire_gate(
            dir.path(),
            "./unknown.sh",
            &GateSchedule::default(),
            Duration::from_millis(10),
            &cancel
        )
        .await,
        Err(GateWaitError::TimedOut)
    ));
    drop(first);
    acquire_gate(
        dir.path(),
        "./unknown.sh",
        &GateSchedule::default(),
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
    let first = acquire_gate(
        dir.path(),
        "cargo test",
        &GateSchedule::default(),
        Duration::from_secs(1),
        &cancel,
    )
    .await
    .unwrap();
    let root = dir.path().to_owned();
    let job = tokio::spawn(async move {
        acquire_gate(
            &root,
            "./opaque.sh",
            &GateSchedule::default(),
            Duration::from_secs(5),
            &CancellationToken::new(),
        )
        .await
    });
    tokio::task::yield_now().await;
    job.abort();
    let _ = job.await;
    drop(first);
    acquire_gate(
        dir.path(),
        "cargo test",
        &GateSchedule::default(),
        Duration::from_secs(1),
        &cancel,
    )
    .await
    .unwrap();
}
