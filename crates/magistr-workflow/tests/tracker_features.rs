//! Grouping: tasks feature into containers.
//!
//! Driven through `project_all` against `FakeTracker`, so these assert the
//! PROJECTOR's behaviour rather than one adapter's. The Linear half is the same
//! contract with a network under it.

use magistr_workflow::plan::domain::TaskStatus;
use magistr_workflow::plan::engine::PlanEngine;
use magistr_workflow::tracker::domain::FeatureState;
use magistr_workflow::tracker::fake::FakeTracker;
use magistr_workflow::tracker::project::project_all;

const PROVIDER: &str = "fake";

fn engine_with(tasks: &[(&str, Option<&str>, TaskStatus)]) -> PlanEngine {
    let mut e = PlanEngine::new("proj".into());
    for (id, feature, status) in tasks {
        e.add_task(
            (*id).into(),
            format!("Task {id}"),
            *status,
            10,
            format!("why {id}"),
            vec![],
            vec![],
            false,
        )
        .expect("add");
        e.set_tracker_opt_in(id, PROVIDER, true).expect("opt in");
        if let Some(g) = feature {
            e.set_feature(id, Some((*g).into())).expect("feature");
        }
    }
    e
}

/// THE promise: one container per feature, and every grouped issue filed in its own.
#[tokio::test]
async fn each_feature_becomes_one_container_and_its_items_carry_it() {
    let mut e = engine_with(&[
        ("tracker-a", Some("Harbour master"), TaskStatus::ToDo),
        ("tracker-b", Some("Harbour master"), TaskStatus::ToDo),
        ("saas-a", Some("Container yard"), TaskStatus::ToDo),
    ]);
    let t = FakeTracker::new(PROVIDER);

    let report = project_all(&mut e, &t, None).await.expect("project");

    assert_eq!(
        report.features_ensured,
        vec!["Container yard".to_string(), "Harbour master".to_string()],
        "one container per DISTINCT feature, not one per task"
    );
    assert_eq!(t.feature_writes(), 2, "two containers, two writes");

    for item in t.items() {
        let expected = if item.title.contains("tracker") {
            "Harbour master"
        } else {
            "Container yard"
        };
        assert_eq!(
            item.feature.as_deref(),
            Some(expected),
            "every item must carry its container: {item:?}"
        );
    }
}

/// The regression that matters most: adding grouping must not change what
/// happens to a task that has none.
#[tokio::test]
async fn an_unassigned_task_projects_exactly_as_before() {
    let mut e = engine_with(&[("loner", None, TaskStatus::ToDo)]);
    let t = FakeTracker::new(PROVIDER);

    let report = project_all(&mut e, &t, None).await.expect("project");

    assert!(
        report.features_ensured.is_empty(),
        "no feature means NO container is invented for it"
    );
    assert_eq!(t.feature_writes(), 0);
    assert_eq!(t.items().len(), 1, "the issue is still created");
    assert_eq!(
        t.items()[0].feature,
        None,
        "and it is filed flat, exactly as every item was before containers existed"
    );
}

/// Called on every push for every feature, so an unchanged container must cost
/// nothing — otherwise the cadence churns the workspace forever.
#[tokio::test]
async fn a_second_push_rewrites_no_container() {
    let mut e = engine_with(&[("tracker-a", Some("Harbour master"), TaskStatus::ToDo)]);
    let t = FakeTracker::new(PROVIDER);

    project_all(&mut e, &t, None).await.expect("first");
    let after_first = t.feature_writes();
    project_all(&mut e, &t, None).await.expect("second");

    assert_eq!(
        t.feature_writes(),
        after_first,
        "an unchanged container must be a no-op on re-push, not a re-write"
    );
    assert_eq!(t.features().len(), 1, "and must not be duplicated");
}

/// State is DERIVED, and the derivation is the only opinion we assert upstream.
#[tokio::test]
async fn container_state_is_derived_from_its_tasks() {
    // All pending -> nothing has started.
    let mut e = engine_with(&[("a-1", Some("Quayside"), TaskStatus::ToDo)]);
    let t = FakeTracker::new(PROVIDER);
    project_all(&mut e, &t, None).await.expect("project");
    assert_eq!(
        t.features(),
        vec![("Quayside".to_string(), FeatureState::NotStarted)]
    );

    // One in progress -> the feature is moving.
    let mut e = engine_with(&[
        ("b-1", Some("Paperwork"), TaskStatus::ToDo),
        ("b-2", Some("Paperwork"), TaskStatus::InProgress),
    ]);
    let t = FakeTracker::new(PROVIDER);
    project_all(&mut e, &t, None).await.expect("project");
    assert_eq!(
        t.features(),
        vec![("Paperwork".to_string(), FeatureState::Active)]
    );

    // Every task done -> finished.
    let mut e = engine_with(&[("c-1", Some("Cold store"), TaskStatus::ToDo)]);
    e.set_status("c-1", TaskStatus::InProgress).expect("start");
    e.set_status("c-1", TaskStatus::Done).expect("finish");
    let t = FakeTracker::new(PROVIDER);
    project_all(&mut e, &t, None).await.expect("project");
    assert_eq!(
        t.features(),
        vec![("Cold store".to_string(), FeatureState::Complete)]
    );
}

/// The retired filter, tested where it actually DISCRIMINATES.
///
/// The first version of this asserted an all-retired feature is NotStarted —
/// which is true whether or not retired tasks are filtered, so deliberately
/// removing the filter still passed it. The case that separates them is a MIXED
/// feature: with the filter the abandoned task is ignored and the rest being
/// done makes the feature done; without it, the feature is stuck Active forever
/// because one task will never reach Done.
#[tokio::test]
async fn a_retired_task_does_not_hold_its_feature_open_forever() {
    let mut e = engine_with(&[
        ("d-1", Some("Dry dock"), TaskStatus::ToDo),
        ("d-2", Some("Dry dock"), TaskStatus::Idea),
    ]);
    e.set_status("d-1", TaskStatus::InProgress).expect("start");
    e.set_status("d-1", TaskStatus::Done).expect("finish");
    e.set_status("d-2", TaskStatus::Retired).expect("abandon");
    let t = FakeTracker::new(PROVIDER);

    project_all(&mut e, &t, None).await.expect("project");

    assert_eq!(
        t.features(),
        vec![("Dry dock".to_string(), FeatureState::Complete)],
        "the abandoned task must be IGNORED, not treated as unfinished work \
         that keeps the feature open forever"
    );
}

/// The bug the LIVE workspace found that the fake never could: every real
/// plan grows its features AFTER its issues exist. The first push records a
/// content hash; seeding a feature changes nothing that hash covered, so the
/// cheap skip ate the projectId attach and every container stayed empty.
/// A group-only change must dirty the item.
#[tokio::test]
async fn grouping_an_already_pushed_task_reaches_the_tracker() {
    let mut e = engine_with(&[("late-1", None, TaskStatus::ToDo)]);
    let t = FakeTracker::new(PROVIDER);

    project_all(&mut e, &t, None).await.expect("first push");
    assert_eq!(t.items()[0].feature, None, "precondition: filed flat");

    e.set_feature("late-1", Some("Night shift".into()))
        .expect("feature");
    project_all(&mut e, &t, None).await.expect("second push");

    assert_eq!(
        t.items()[0].feature.as_deref(),
        Some("Night shift"),
        "a feature set after the first push must reach the tracker, \
         not be skipped as an unchanged item"
    );
}

/// The other half: abandoning EVERYTHING is not the same as finishing it.
#[tokio::test]
async fn an_all_retired_feature_is_not_reported_complete() {
    let mut e = engine_with(&[("g-1", Some("Check house"), TaskStatus::Idea)]);
    e.set_status("g-1", TaskStatus::Retired).expect("obsolete");
    let t = FakeTracker::new(PROVIDER);

    project_all(&mut e, &t, None).await.expect("project");

    assert_eq!(
        t.features(),
        vec![("Check house".to_string(), FeatureState::NotStarted)],
        "a feature whose every task was abandoned has not been completed"
    );
}
