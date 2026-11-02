use super::*;
use crate::plan::WaitingReason;

fn task(id: &str, status: TaskStatus, feature: Option<&str>) -> Task {
    serde_json::from_value(serde_json::json!({
        "id": id, "name": id, "title": format!("Work {id}"),
        "status": status, "priority": 10, "feature": feature,
        "description": "", "notes": "", "acceptance": [], "deps": [],
        "refs": [], "shared": false,
        "created_at": "2026-10-14T00:00:00Z", "updated_at": "2026-10-14T00:00:00Z"
    }))
    .expect("complete task")
}

fn blocker() -> WaitingOn {
    WaitingOn {
        kind: WaitingReason::Outside,
        reason: "Access is needed.".into(),
        evidence: Some("think:4".into()),
        waiting_since: "2026-10-14T00:00:00Z".into(),
    }
}

#[test]
fn an_empty_catalog_has_explicit_unassigned_buckets_and_no_focus() {
    let catalog = workstream_catalog(&[]);
    assert!(catalog.current.is_empty());
    assert!(catalog.history.is_empty());
    assert_eq!(catalog.unassigned, WorkstreamActivity::default());
    assert_eq!(catalog.unassigned_history, WorkstreamHistory::default());
    assert_eq!(catalog.focus_state(None), FocusState::Unset);
    assert!(catalog.authored_names().is_empty());
    assert_eq!(
        catalog.resolve_current("Anything"),
        WorkstreamResolution::Unknown { choices: vec![] }
    );
}

#[test]
fn current_activity_and_terminal_history_share_names_without_sharing_counts() {
    let tasks = vec![
        task("idea", TaskStatus::Idea, Some("Mixed")),
        task("done-first", TaskStatus::Done, Some("Mixed")),
        task("to_do", TaskStatus::ToDo, Some("Mixed")),
        task("working", TaskStatus::InProgress, Some("Mixed")),
        task("waiting", TaskStatus::Waiting, Some("Mixed")),
        task("obsolete", TaskStatus::Retired, Some("Mixed")),
        task("done-last", TaskStatus::Done, Some("Mixed")),
        task("old", TaskStatus::Done, Some("Closed")),
        task("loose", TaskStatus::ToDo, None),
        task("loose-old", TaskStatus::Retired, None),
    ];
    let catalog = workstream_catalog(&tasks);
    let activity = &catalog.current["Mixed"];
    assert_eq!(
        activity.counts,
        WorkstreamCounts {
            total: 4,
            idea: 1,
            to_do: 1,
            in_progress: 1,
            waiting: 1
        }
    );
    assert_eq!(
        activity.readiness,
        WorkstreamReadinessCounts {
            ready: 1,
            dependent: 0,
            waiting: 1
        }
    );
    assert_eq!(activity.next_ready.as_deref(), Some("to_do"));
    assert_eq!(catalog.history["Mixed"].done, 2);
    assert_eq!(catalog.history["Mixed"].retired, 1);
    assert_eq!(
        workstream_history(&tasks, Some("Mixed")).task_ids,
        ["done-first", "obsolete", "done-last"]
    );
    assert!(!catalog.current.contains_key("Closed"));
    assert_eq!(catalog.current_names(), ["Mixed"]);
    assert_eq!(catalog.authored_names(), ["Closed", "Mixed"]);
    assert_eq!(catalog.unassigned.counts.total, 1);
    assert_eq!(catalog.unassigned.next_ready.as_deref(), Some("loose"));
    assert_eq!(workstream_history(&tasks, None).task_ids, ["loose-old"]);
    let frontier = workstream_frontier(&tasks, Some("Mixed"));
    assert_eq!(frontier.activity, *activity);
    assert_eq!(
        frontier
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        ["idea", "to_do", "working", "waiting"]
    );
}

#[test]
fn dependencies_are_assessed_against_the_complete_snapshot() {
    let mut waiting = task("waiting", TaskStatus::ToDo, Some("Delivery"));
    waiting.deps = vec![
        "done".into(),
        "obsolete".into(),
        "missing".into(),
        "working".into(),
    ];
    let tasks = vec![
        waiting,
        task("done", TaskStatus::Done, Some("Foundation")),
        task("obsolete", TaskStatus::Retired, Some("Foundation")),
        task("working", TaskStatus::InProgress, None),
    ];
    let frontier = workstream_frontier(&tasks, Some("Delivery"));
    assert_eq!(frontier.activity.readiness.dependent, 1);
    assert_eq!(frontier.activity.next_ready, None);
    assert_eq!(
        frontier.items[0].readiness,
        WorkstreamReadiness::Dependent {
            dependencies: vec![
                WorkstreamDependency {
                    id: "obsolete".into(),
                    status: Some(TaskStatus::Retired)
                },
                WorkstreamDependency {
                    id: "missing".into(),
                    status: None
                },
                WorkstreamDependency {
                    id: "working".into(),
                    status: Some(TaskStatus::InProgress)
                },
            ],
        }
    );
    let mut satisfied = tasks.clone();
    satisfied[0].deps = vec!["done".into()];
    assert_eq!(
        workstream_frontier(&satisfied, Some("Delivery")).items[0].readiness,
        WorkstreamReadiness::Ready
    );
}

#[test]
fn explicit_blockers_take_precedence_and_unexplained_blocked_status_stays_honest() {
    let mut pending = task("to_do", TaskStatus::ToDo, Some("Delivery"));
    pending.deps = vec!["missing".into()];
    pending.waiting_on = Some(blocker());
    let mut historical = task("old", TaskStatus::Done, Some("Delivery"));
    historical.waiting_on = Some(blocker());
    let tasks = vec![
        pending,
        task("waiting", TaskStatus::Waiting, Some("Delivery")),
        historical,
    ];
    let catalog = workstream_catalog(&tasks);
    let frontier = workstream_frontier(&tasks, Some("Delivery"));
    assert_eq!(frontier.activity.counts.waiting, 1);
    assert_eq!(frontier.activity.readiness.waiting, 2);
    assert_eq!(frontier.activity.readiness.dependent, 0);
    assert_eq!(catalog.history["Delivery"].done, 1);
    assert_eq!(
        frontier.items[0].readiness,
        WorkstreamReadiness::Waiting {
            waiting_on: Some(blocker())
        }
    );
    assert_eq!(
        frontier.items[1].readiness,
        WorkstreamReadiness::Waiting { waiting_on: None }
    );
    let wire = serde_json::to_value(&frontier.items[1]).unwrap();
    assert_eq!(
        wire["readiness"],
        serde_json::json!({"state": "waiting", "waiting_on": null})
    );
}

#[test]
fn the_lowest_ready_priority_wins_and_equal_priorities_keep_insertion_order() {
    let mut blocked = task("waiting", TaskStatus::ToDo, Some("Delivery"));
    blocked.priority = 0;
    blocked.waiting_on = Some(blocker());
    let mut first = task("z-first", TaskStatus::ToDo, Some("Delivery"));
    first.priority = 3;
    let mut second = task("a-second", TaskStatus::ToDo, Some("Delivery"));
    second.priority = 3;
    let tasks = vec![
        blocked,
        task("earlier-but-lower", TaskStatus::ToDo, Some("Delivery")),
        first,
        second,
    ];
    let frontier = workstream_frontier(&tasks, Some("Delivery"));
    assert_eq!(frontier.activity.next_ready.as_deref(), Some("z-first"));
    assert_eq!(
        workstream_catalog(&tasks).current["Delivery"],
        frontier.activity
    );
}

#[test]
fn unassigned_is_exact_and_never_a_synthetic_or_global_workstream() {
    let tasks = vec![
        task("loose", TaskStatus::ToDo, None),
        task("named-unassigned", TaskStatus::ToDo, Some("Unassigned")),
        task("empty-name", TaskStatus::Done, Some("")),
        task("spaced-name", TaskStatus::Idea, Some(" Work ")),
    ];
    let catalog = workstream_catalog(&tasks);
    assert_eq!(catalog.current_names(), [" Work ", "Unassigned"]);
    assert!(catalog.history.contains_key(""));
    assert_eq!(catalog.unassigned.counts.total, 1);
    assert_eq!(workstream_frontier(&tasks, None).items[0].id, "loose");
    assert_eq!(
        workstream_frontier(&tasks, Some("Unassigned")).items[0].id,
        "named-unassigned"
    );
    assert!(
        workstream_frontier(&tasks, Some("Missing"))
            .items
            .is_empty()
    );
    assert!(workstream_frontier(&tasks, Some("")).items.is_empty());
}

#[test]
fn exact_closed_selection_never_falls_through_to_a_current_fragment() {
    let tasks = vec![
        task("old", TaskStatus::Done, Some("Rust")),
        task("desktop", TaskStatus::ToDo, Some("Rust desktop")),
        task("billing", TaskStatus::Idea, Some("Billing")),
    ];
    let catalog = workstream_catalog(&tasks);
    let refusal = catalog.resolve_current("Rust");
    assert_eq!(
        refusal,
        WorkstreamResolution::Closed {
            feature: "Rust".into(),
            choices: vec!["Billing".into(), "Rust desktop".into()],
        }
    );
    assert!(refusal.explain("Rust").contains("no unfinished work"));
    assert_eq!(catalog.resolve_current("rust"), refusal);
    assert_eq!(
        catalog.resolve_current("desktop").current(),
        Some("Rust desktop")
    );
    assert_eq!(
        catalog.resolve_current("RUST DESKTOP").current(),
        Some("Rust desktop")
    );
    assert_eq!(
        catalog.resolve_current(" Billing ").current(),
        Some("Billing")
    );
    assert_eq!(
        catalog.focus_state(Some("Rust")),
        FocusState::Closed {
            feature: "Rust".into()
        }
    );
    assert_eq!(
        catalog.focus_state(Some("rust")),
        FocusState::Missing {
            feature: "rust".into()
        }
    );
}

#[test]
fn saved_focus_is_exact_even_when_one_current_name_contains_it() {
    let tasks = vec![task("desktop", TaskStatus::ToDo, Some("Rust desktop"))];
    let catalog = workstream_catalog(&tasks);
    assert_eq!(
        catalog.resolve_current("Rust").current(),
        Some("Rust desktop")
    );
    assert_eq!(
        catalog.focus_state(Some("Rust")),
        FocusState::Missing {
            feature: "Rust".into()
        }
    );
    assert_eq!(
        catalog.focus_state(Some("Rust desktop")),
        FocusState::Current {
            feature: "Rust desktop".into()
        }
    );
}

#[test]
fn current_resolution_reports_ambiguity_without_offering_terminal_only_names() {
    let tasks = vec![
        task("old", TaskStatus::Retired, Some("Billing history")),
        task("core", TaskStatus::ToDo, Some("Billing core")),
        task("reports", TaskStatus::InProgress, Some("Billing reports")),
    ];
    let catalog = workstream_catalog(&tasks);
    assert_eq!(
        catalog.resolve_current("billing"),
        WorkstreamResolution::Ambiguous {
            matches: vec!["Billing core".into(), "Billing reports".into()]
        }
    );
    assert_eq!(
        catalog.resolve_current("missing"),
        WorkstreamResolution::Unknown {
            choices: vec!["Billing core".into(), "Billing reports".into()]
        }
    );
    assert_eq!(
        catalog.resolve_current(""),
        WorkstreamResolution::Unknown {
            choices: catalog.current_names()
        }
    );
}

#[test]
fn snapshot_projection_recomputes_after_completion_regrouping_and_reopening() {
    let mut tasks = vec![task("work", TaskStatus::ToDo, Some("Delivery"))];
    let snapshot = |tasks: &[Task]| crate::NativeSnapshot {
        plan: crate::ReadState::Current(crate::Observed {
            value: crate::PlanData {
                tasks: tasks.to_vec(),
                ..Default::default()
            },
            source: crate::ReadSource::Memory,
        }),
        ..Default::default()
    };
    assert_eq!(
        snapshot(&tasks)
            .workstreams()
            .current()
            .unwrap()
            .value
            .current_names(),
        ["Delivery"]
    );
    tasks[0].status = TaskStatus::Done;
    assert!(
        snapshot(&tasks)
            .workstreams()
            .current()
            .unwrap()
            .value
            .current
            .is_empty()
    );
    assert_eq!(
        workstream_history(&tasks, Some("Delivery")).task_ids,
        ["work"]
    );
    tasks[0].feature = Some("Archive".into());
    assert_eq!(
        snapshot(&tasks)
            .workstreams()
            .current()
            .unwrap()
            .value
            .focus_state(Some("Delivery")),
        FocusState::Missing {
            feature: "Delivery".into()
        }
    );
    tasks[0].status = TaskStatus::InProgress;
    let observation = snapshot(&tasks).workstreams();
    let catalog = &observation.current().unwrap().value;
    assert_eq!(catalog.current_names(), ["Archive"]);
    assert!(catalog.history.is_empty());
    assert_eq!(
        serde_json::from_value::<WorkstreamCatalog>(serde_json::to_value(catalog).unwrap())
            .unwrap(),
        *catalog
    );
    assert_eq!(
        serde_json::to_value(catalog.focus_state(Some("Archive"))).unwrap(),
        serde_json::json!({"state": "current", "feature": "Archive"})
    );
}

#[test]
fn case_insensitive_authored_identity_precedes_current_fragment_search() {
    let tasks = vec![
        task("closed", TaskStatus::Done, Some("Rust")),
        task("current", TaskStatus::ToDo, Some("RUST")),
        task("desktop", TaskStatus::ToDo, Some("Rust desktop")),
    ];
    let catalog = workstream_catalog(&tasks);
    assert!(matches!(
        catalog.resolve_current("Rust"),
        WorkstreamResolution::Closed { .. }
    ));
    assert_eq!(catalog.resolve_current("RUST").current(), Some("RUST"));
    assert_eq!(
        catalog.resolve_current("rUsT"),
        WorkstreamResolution::Ambiguous {
            matches: vec!["RUST".into(), "Rust".into()]
        }
    );
}

#[test]
fn catalog_history_stays_bounded_and_named_inspection_exposes_records() {
    let tasks: Vec<_> = (0..1000)
        .map(|index| {
            task(
                &format!("finished-{index}"),
                TaskStatus::Done,
                Some("Archive"),
            )
        })
        .collect();
    let catalog = workstream_catalog(&tasks);
    let value = serde_json::to_value(&catalog).unwrap();
    assert_eq!(
        value["history"]["Archive"],
        serde_json::json!({"done": 1000, "retired": 0})
    );
    assert!(value["unassigned_history"].get("task_ids").is_none());
    assert!(serde_json::to_string(&catalog).unwrap().len() < 500);
    let history = workstream_history(&tasks, Some("Archive"));
    assert_eq!(history.counts, catalog.history["Archive"]);
    assert_eq!(history.task_ids.len(), 1000);
    assert_eq!(history.task_ids[0], "finished-0");
    assert_eq!(history.task_ids[999], "finished-999");
    let inspected = serde_json::to_value(&history).unwrap();
    assert_eq!(inspected["done"], 1000);
    assert!(inspected.get("counts").is_none());
    assert_eq!(inspected["task_ids"].as_array().unwrap().len(), 1000);
    assert_eq!(
        workstream_history(&tasks, Some("Missing")),
        WorkstreamHistoryRecords::default()
    );
    assert_eq!(
        workstream_history(&tasks, None),
        WorkstreamHistoryRecords::default()
    );
}
