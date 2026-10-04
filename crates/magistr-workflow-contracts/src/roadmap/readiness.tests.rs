use super::*;

#[test]
fn only_pending_with_completed_dependencies_and_no_blocker_is_ready() {
    let deps = vec!["ground".into()];
    for status in [
        ChunkStatus::Backlog,
        ChunkStatus::InProgress,
        ChunkStatus::Blocked,
        ChunkStatus::Done,
        ChunkStatus::Obsoleted,
    ] {
        assert!(!assess_readiness(status, None, &deps, |_| Some(ChunkStatus::Done)).is_ready());
    }
    assert!(
        assess_readiness(ChunkStatus::Pending, None, &deps, |_| Some(
            ChunkStatus::Done
        ))
        .is_ready()
    );
    assert_eq!(
        assess_readiness(
            ChunkStatus::Pending,
            Some("Access is needed."),
            &deps,
            |_| Some(ChunkStatus::Done)
        ),
        Readiness::Blocked("Access is needed.")
    );
}

#[test]
fn unmet_and_missing_dependencies_keep_their_identity_and_status() {
    let deps = vec!["complete".into(), "working".into(), "absent".into()];
    let readiness = assess_readiness(ChunkStatus::Pending, None, &deps, |id| match id {
        "complete" => Some(ChunkStatus::Done),
        "working" => Some(ChunkStatus::InProgress),
        _ => None,
    });
    assert_eq!(
        readiness,
        Readiness::Waiting(vec![
            WaitingDependency {
                id: "working",
                status: Some(ChunkStatus::InProgress)
            },
            WaitingDependency {
                id: "absent",
                status: None
            },
        ])
    );
    for status in [
        ChunkStatus::Backlog,
        ChunkStatus::Pending,
        ChunkStatus::InProgress,
        ChunkStatus::Blocked,
        ChunkStatus::Obsoleted,
    ] {
        assert!(!assess_readiness(ChunkStatus::Pending, None, &deps, |_| Some(status)).is_ready());
    }
}

#[test]
fn historical_blockers_do_not_replace_terminal_lifecycle_status() {
    for status in [ChunkStatus::Done, ChunkStatus::Obsoleted] {
        assert_eq!(
            assess_readiness(status, Some("Former blocker."), &[], |_| None),
            Readiness::Inactive(status)
        );
    }
}

#[test]
fn equal_priorities_keep_record_order_and_ineligible_records_are_skipped() {
    let rows = [
        ("blocked", 0, false),
        ("z-first", 10, true),
        ("a-second", 10, true),
    ];
    assert_eq!(
        next_ready(&rows, |c| c.2, |c| c.1).map(|c| c.0),
        Some("z-first")
    );
    assert!(next_ready(&rows, |_| false, |c| c.1).is_none());
}
