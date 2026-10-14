use super::*;
use crate::roadmap::BlockerKind;

fn chunk(id: &str, status: ChunkStatus, group: Option<&str>) -> Chunk {
    serde_json::from_value(serde_json::json!({
        "id": id, "name": id, "title": format!("Work {id}"),
        "status": status, "priority": 10, "group": group,
        "description": "", "notes": "", "acceptance": [], "deps": [],
        "cross_refs": [], "shared": false,
        "created_at": "2026-10-14T00:00:00Z", "updated_at": "2026-10-14T00:00:00Z"
    }))
    .expect("complete chunk")
}

fn blocker() -> BlockedBy {
    BlockedBy {
        kind: BlockerKind::External,
        reason: "Access is needed.".into(),
        evidence: Some("think:4".into()),
        blocked_at: "2026-10-14T00:00:00Z".into(),
    }
}

#[test]
fn an_empty_catalog_has_explicit_ungrouped_buckets_and_no_focus() {
    let catalog = workstream_catalog(&[]);
    assert!(catalog.current.is_empty());
    assert!(catalog.history.is_empty());
    assert_eq!(catalog.ungrouped, WorkstreamActivity::default());
    assert_eq!(catalog.ungrouped_history, WorkstreamHistory::default());
    assert_eq!(catalog.focus_state(None), FocusState::Unset);
    assert!(catalog.authored_names().is_empty());
    assert_eq!(
        catalog.resolve_current("Anything"),
        WorkstreamResolution::Unknown { choices: vec![] }
    );
}

#[test]
fn current_activity_and_terminal_history_share_names_without_sharing_counts() {
    let chunks = vec![
        chunk("backlog", ChunkStatus::Backlog, Some("Mixed")),
        chunk("done-first", ChunkStatus::Done, Some("Mixed")),
        chunk("pending", ChunkStatus::Pending, Some("Mixed")),
        chunk("working", ChunkStatus::InProgress, Some("Mixed")),
        chunk("blocked", ChunkStatus::Blocked, Some("Mixed")),
        chunk("obsolete", ChunkStatus::Obsoleted, Some("Mixed")),
        chunk("done-last", ChunkStatus::Done, Some("Mixed")),
        chunk("old", ChunkStatus::Done, Some("Closed")),
        chunk("loose", ChunkStatus::Pending, None),
        chunk("loose-old", ChunkStatus::Obsoleted, None),
    ];
    let catalog = workstream_catalog(&chunks);
    let activity = &catalog.current["Mixed"];
    assert_eq!(
        activity.counts,
        WorkstreamCounts {
            total: 4,
            backlog: 1,
            pending: 1,
            in_progress: 1,
            blocked: 1
        }
    );
    assert_eq!(
        activity.readiness,
        WorkstreamReadinessCounts {
            ready: 1,
            waiting: 0,
            blocked: 1
        }
    );
    assert_eq!(activity.next_ready.as_deref(), Some("pending"));
    assert_eq!(catalog.history["Mixed"].done, 2);
    assert_eq!(catalog.history["Mixed"].obsoleted, 1);
    assert_eq!(
        workstream_history(&chunks, Some("Mixed")).chunk_ids,
        ["done-first", "obsolete", "done-last"]
    );
    assert!(!catalog.current.contains_key("Closed"));
    assert_eq!(catalog.current_names(), ["Mixed"]);
    assert_eq!(catalog.authored_names(), ["Closed", "Mixed"]);
    assert_eq!(catalog.ungrouped.counts.total, 1);
    assert_eq!(catalog.ungrouped.next_ready.as_deref(), Some("loose"));
    assert_eq!(workstream_history(&chunks, None).chunk_ids, ["loose-old"]);
    let frontier = workstream_frontier(&chunks, Some("Mixed"));
    assert_eq!(frontier.activity, *activity);
    assert_eq!(
        frontier
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        ["backlog", "pending", "working", "blocked"]
    );
}

#[test]
fn dependencies_are_assessed_against_the_complete_snapshot() {
    let mut waiting = chunk("waiting", ChunkStatus::Pending, Some("Delivery"));
    waiting.deps = vec![
        "done".into(),
        "obsolete".into(),
        "missing".into(),
        "working".into(),
    ];
    let chunks = vec![
        waiting,
        chunk("done", ChunkStatus::Done, Some("Foundation")),
        chunk("obsolete", ChunkStatus::Obsoleted, Some("Foundation")),
        chunk("working", ChunkStatus::InProgress, None),
    ];
    let frontier = workstream_frontier(&chunks, Some("Delivery"));
    assert_eq!(frontier.activity.readiness.waiting, 1);
    assert_eq!(frontier.activity.next_ready, None);
    assert_eq!(
        frontier.items[0].readiness,
        WorkstreamReadiness::Waiting {
            dependencies: vec![
                WorkstreamDependency {
                    id: "obsolete".into(),
                    status: Some(ChunkStatus::Obsoleted)
                },
                WorkstreamDependency {
                    id: "missing".into(),
                    status: None
                },
                WorkstreamDependency {
                    id: "working".into(),
                    status: Some(ChunkStatus::InProgress)
                },
            ],
        }
    );
    let mut satisfied = chunks.clone();
    satisfied[0].deps = vec!["done".into()];
    assert_eq!(
        workstream_frontier(&satisfied, Some("Delivery")).items[0].readiness,
        WorkstreamReadiness::Ready
    );
}

#[test]
fn explicit_blockers_take_precedence_and_unexplained_blocked_status_stays_honest() {
    let mut pending = chunk("pending", ChunkStatus::Pending, Some("Delivery"));
    pending.deps = vec!["missing".into()];
    pending.blocked_by = Some(blocker());
    let mut historical = chunk("old", ChunkStatus::Done, Some("Delivery"));
    historical.blocked_by = Some(blocker());
    let chunks = vec![
        pending,
        chunk("blocked", ChunkStatus::Blocked, Some("Delivery")),
        historical,
    ];
    let catalog = workstream_catalog(&chunks);
    let frontier = workstream_frontier(&chunks, Some("Delivery"));
    assert_eq!(frontier.activity.counts.blocked, 1);
    assert_eq!(frontier.activity.readiness.blocked, 2);
    assert_eq!(frontier.activity.readiness.waiting, 0);
    assert_eq!(catalog.history["Delivery"].done, 1);
    assert_eq!(
        frontier.items[0].readiness,
        WorkstreamReadiness::Blocked {
            blocker: Some(blocker())
        }
    );
    assert_eq!(
        frontier.items[1].readiness,
        WorkstreamReadiness::Blocked { blocker: None }
    );
    let wire = serde_json::to_value(&frontier.items[1]).unwrap();
    assert_eq!(
        wire["readiness"],
        serde_json::json!({"state": "blocked", "blocker": null})
    );
}

#[test]
fn the_lowest_ready_priority_wins_and_equal_priorities_keep_insertion_order() {
    let mut blocked = chunk("blocked", ChunkStatus::Pending, Some("Delivery"));
    blocked.priority = 0;
    blocked.blocked_by = Some(blocker());
    let mut first = chunk("z-first", ChunkStatus::Pending, Some("Delivery"));
    first.priority = 3;
    let mut second = chunk("a-second", ChunkStatus::Pending, Some("Delivery"));
    second.priority = 3;
    let chunks = vec![
        blocked,
        chunk("earlier-but-lower", ChunkStatus::Pending, Some("Delivery")),
        first,
        second,
    ];
    let frontier = workstream_frontier(&chunks, Some("Delivery"));
    assert_eq!(frontier.activity.next_ready.as_deref(), Some("z-first"));
    assert_eq!(
        workstream_catalog(&chunks).current["Delivery"],
        frontier.activity
    );
}

#[test]
fn ungrouped_is_exact_and_never_a_synthetic_or_global_workstream() {
    let chunks = vec![
        chunk("loose", ChunkStatus::Pending, None),
        chunk("named-ungrouped", ChunkStatus::Pending, Some("Ungrouped")),
        chunk("empty-name", ChunkStatus::Done, Some("")),
        chunk("spaced-name", ChunkStatus::Backlog, Some(" Work ")),
    ];
    let catalog = workstream_catalog(&chunks);
    assert_eq!(catalog.current_names(), [" Work ", "Ungrouped"]);
    assert!(catalog.history.contains_key(""));
    assert_eq!(catalog.ungrouped.counts.total, 1);
    assert_eq!(workstream_frontier(&chunks, None).items[0].id, "loose");
    assert_eq!(
        workstream_frontier(&chunks, Some("Ungrouped")).items[0].id,
        "named-ungrouped"
    );
    assert!(
        workstream_frontier(&chunks, Some("Missing"))
            .items
            .is_empty()
    );
    assert!(workstream_frontier(&chunks, Some("")).items.is_empty());
}

#[test]
fn exact_closed_selection_never_falls_through_to_a_current_fragment() {
    let chunks = vec![
        chunk("old", ChunkStatus::Done, Some("Rust")),
        chunk("desktop", ChunkStatus::Pending, Some("Rust desktop")),
        chunk("billing", ChunkStatus::Backlog, Some("Billing")),
    ];
    let catalog = workstream_catalog(&chunks);
    let refusal = catalog.resolve_current("Rust");
    assert_eq!(
        refusal,
        WorkstreamResolution::Closed {
            group: "Rust".into(),
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
            group: "Rust".into()
        }
    );
    assert_eq!(
        catalog.focus_state(Some("rust")),
        FocusState::Missing {
            group: "rust".into()
        }
    );
}

#[test]
fn saved_focus_is_exact_even_when_one_current_name_contains_it() {
    let chunks = vec![chunk("desktop", ChunkStatus::Pending, Some("Rust desktop"))];
    let catalog = workstream_catalog(&chunks);
    assert_eq!(
        catalog.resolve_current("Rust").current(),
        Some("Rust desktop")
    );
    assert_eq!(
        catalog.focus_state(Some("Rust")),
        FocusState::Missing {
            group: "Rust".into()
        }
    );
    assert_eq!(
        catalog.focus_state(Some("Rust desktop")),
        FocusState::Current {
            group: "Rust desktop".into()
        }
    );
}

#[test]
fn current_resolution_reports_ambiguity_without_offering_terminal_only_names() {
    let chunks = vec![
        chunk("old", ChunkStatus::Obsoleted, Some("Billing history")),
        chunk("core", ChunkStatus::Pending, Some("Billing core")),
        chunk("reports", ChunkStatus::InProgress, Some("Billing reports")),
    ];
    let catalog = workstream_catalog(&chunks);
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
    let mut snapshot = crate::NativeSnapshot {
        chunks: vec![chunk("work", ChunkStatus::Pending, Some("Delivery"))],
        ..Default::default()
    };
    assert_eq!(snapshot.workstreams().current_names(), ["Delivery"]);
    snapshot.chunks[0].status = ChunkStatus::Done;
    assert!(snapshot.workstreams().current.is_empty());
    assert_eq!(
        workstream_history(&snapshot.chunks, Some("Delivery")).chunk_ids,
        ["work"]
    );
    snapshot.chunks[0].group = Some("Archive".into());
    assert_eq!(
        snapshot.workstreams().focus_state(Some("Delivery")),
        FocusState::Missing {
            group: "Delivery".into()
        }
    );
    snapshot.chunks[0].status = ChunkStatus::InProgress;
    let catalog = snapshot.workstreams();
    assert_eq!(catalog.current_names(), ["Archive"]);
    assert!(catalog.history.is_empty());
    assert_eq!(
        serde_json::from_value::<WorkstreamCatalog>(serde_json::to_value(&catalog).unwrap())
            .unwrap(),
        catalog
    );
    let focus = catalog.focus_state(Some("Archive"));
    assert_eq!(
        serde_json::to_value(focus).unwrap(),
        serde_json::json!({"state": "current", "group": "Archive"})
    );
}

#[test]
fn case_insensitive_authored_identity_precedes_current_fragment_search() {
    let chunks = vec![
        chunk("closed", ChunkStatus::Done, Some("Rust")),
        chunk("current", ChunkStatus::Pending, Some("RUST")),
        chunk("desktop", ChunkStatus::Pending, Some("Rust desktop")),
    ];
    let catalog = workstream_catalog(&chunks);
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
    let chunks: Vec<_> = (0..1000)
        .map(|index| {
            chunk(
                &format!("finished-{index}"),
                ChunkStatus::Done,
                Some("Archive"),
            )
        })
        .collect();
    let catalog = workstream_catalog(&chunks);
    let value = serde_json::to_value(&catalog).unwrap();
    assert_eq!(
        value["history"]["Archive"],
        serde_json::json!({"done": 1000, "obsoleted": 0})
    );
    assert!(value["ungrouped_history"].get("chunk_ids").is_none());
    assert!(serde_json::to_string(&catalog).unwrap().len() < 500);
    let history = workstream_history(&chunks, Some("Archive"));
    assert_eq!(history.counts, catalog.history["Archive"]);
    assert_eq!(history.chunk_ids.len(), 1000);
    assert_eq!(history.chunk_ids[0], "finished-0");
    assert_eq!(history.chunk_ids[999], "finished-999");
    let inspected = serde_json::to_value(&history).unwrap();
    assert_eq!(inspected["done"], 1000);
    assert!(inspected.get("counts").is_none());
    assert_eq!(inspected["chunk_ids"].as_array().unwrap().len(), 1000);
    assert_eq!(
        workstream_history(&chunks, Some("Missing")),
        WorkstreamHistoryRecords::default()
    );
    assert_eq!(
        workstream_history(&chunks, None),
        WorkstreamHistoryRecords::default()
    );
}
