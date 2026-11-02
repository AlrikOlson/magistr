use super::*;

#[test]
fn only_pending_with_completed_dependencies_and_no_blocker_is_ready() {
    let deps = vec!["ground".into()];
    for status in [
        TaskStatus::Idea,
        TaskStatus::InProgress,
        TaskStatus::Waiting,
        TaskStatus::Done,
        TaskStatus::Retired,
    ] {
        assert!(!assess_readiness(status, None, &deps, |_| Some(TaskStatus::Done)).is_ready());
    }
    assert!(assess_readiness(TaskStatus::ToDo, None, &deps, |_| Some(TaskStatus::Done)).is_ready());
    assert_eq!(
        assess_readiness(
            TaskStatus::ToDo,
            Some("Access is needed."),
            &deps,
            |_| Some(TaskStatus::Done)
        ),
        Readiness::Waiting("Access is needed.")
    );
}

#[test]
fn unmet_and_missing_dependencies_keep_their_identity_and_status() {
    let deps = vec!["complete".into(), "working".into(), "absent".into()];
    let readiness = assess_readiness(TaskStatus::ToDo, None, &deps, |id| match id {
        "complete" => Some(TaskStatus::Done),
        "working" => Some(TaskStatus::InProgress),
        _ => None,
    });
    assert_eq!(
        readiness,
        Readiness::Dependent(vec![
            WaitingDependency {
                id: "working",
                status: Some(TaskStatus::InProgress)
            },
            WaitingDependency {
                id: "absent",
                status: None
            },
        ])
    );
    for status in [
        TaskStatus::Idea,
        TaskStatus::ToDo,
        TaskStatus::InProgress,
        TaskStatus::Waiting,
        TaskStatus::Retired,
    ] {
        assert!(!assess_readiness(TaskStatus::ToDo, None, &deps, |_| Some(status)).is_ready());
    }
}

#[test]
fn historical_blockers_do_not_replace_terminal_lifecycle_status() {
    for status in [TaskStatus::Done, TaskStatus::Retired] {
        assert_eq!(
            assess_readiness(status, Some("Former blocker."), &[], |_| None),
            Readiness::Inactive(status)
        );
    }
}

#[test]
fn equal_priorities_keep_record_order_and_ineligible_records_are_skipped() {
    let rows = [
        ("waiting", 0, false),
        ("z-first", 10, true),
        ("a-second", 10, true),
    ];
    assert_eq!(
        next_ready(&rows, |c| c.2, |c| c.1).map(|c| c.0),
        Some("z-first")
    );
    assert!(next_ready(&rows, |_| false, |c| c.1).is_none());
}
