//! The initiative: the roof above the containers.
//!
//! Driven through `project_all_with_policy` against `FakeTracker`, so these
//! assert the PROJECTOR's behaviour — phase −1 ordering, plan-wide state,
//! degradation — rather than one adapter's. The Linear half (create-once,
//! join-mutation idempotence, the third vocabulary) is in `tracker_linear.rs`
//! against the GraphQL mock.

use magistr_workflow::plan::domain::TaskStatus;
use magistr_workflow::plan::engine::PlanEngine;
use magistr_workflow::tracker::domain::FeatureState;
use magistr_workflow::tracker::fake::FakeTracker;
use magistr_workflow::tracker::ownership::Ownership;
use magistr_workflow::tracker::port::TrackerError;
use magistr_workflow::tracker::project::{project_all, project_all_with_policy};

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

async fn push(
    e: &mut PlanEngine,
    t: &FakeTracker,
    initiative: Option<&str>,
) -> magistr_workflow::tracker::ProjectionReport {
    project_all_with_policy(e, t, None, &Ownership::default(), initiative)
        .await
        .expect("project")
}

/// THE promise: the caller names a roof and every push ensures it, with the
/// state derived from the WHOLE plan.
#[tokio::test]
async fn the_roof_is_ensured_with_the_plans_derived_state() {
    let mut e = engine_with(&[
        ("tracker-a", Some("Harbour master"), TaskStatus::ToDo),
        ("saas-a", Some("Container yard"), TaskStatus::InProgress),
    ]);
    let t = FakeTracker::new(PROVIDER);

    let report = push(&mut e, &t, Some("magistr")).await;

    assert_eq!(
        t.initiative(),
        Some(("magistr".to_string(), FeatureState::Active)),
        "one task moving means the plan is moving"
    );
    assert_eq!(
        report.initiative_ensured.as_deref(),
        Some("magistr"),
        "the report says the roof stands"
    );
    assert!(report.initiative_failure.is_none());
}

/// The regression guard: a caller with no name must change nothing — this is
/// every existing test and every provider without config.
#[tokio::test]
async fn no_roof_is_attempted_without_a_name() {
    let mut e = engine_with(&[("loner", Some("Gate house"), TaskStatus::ToDo)]);
    let t = FakeTracker::new(PROVIDER);

    let report = project_all(&mut e, &t, None).await.expect("project");

    assert_eq!(t.initiative(), None, "no name, no roof");
    assert_eq!(t.initiative_writes(), 0);
    assert!(report.initiative_ensured.is_none());
}

/// Called on every push, so an unchanged roof must cost nothing — the
/// `feature_writes` contract one level up.
#[tokio::test]
async fn a_second_push_rewrites_no_roof() {
    let mut e = engine_with(&[("a-1", Some("Quayside"), TaskStatus::ToDo)]);
    let t = FakeTracker::new(PROVIDER);

    push(&mut e, &t, Some("roof")).await;
    let after_first = t.initiative_writes();
    push(&mut e, &t, Some("roof")).await;

    assert_eq!(
        t.initiative_writes(),
        after_first,
        "an unchanged roof must be a no-op on re-push"
    );
}

/// The roof's state is plan-wide, not an aggregate of the features: an
/// unassigned task in progress moves the plan even though it moves no
/// container. This is the case that separates `derived_state(all planned)`
/// from "derive from the features" — under the latter, the roof here would sit
/// at NotStarted.
#[tokio::test]
async fn an_unassigned_task_still_counts_toward_the_roofs_state() {
    let mut e = engine_with(&[
        ("grouped", Some("Gate house"), TaskStatus::ToDo),
        ("loner", None, TaskStatus::InProgress),
    ]);
    let t = FakeTracker::new(PROVIDER);

    push(&mut e, &t, Some("roof")).await;

    assert_eq!(
        t.features(),
        vec![("Gate house".to_string(), FeatureState::NotStarted)],
        "precondition: the only container has not started"
    );
    assert_eq!(
        t.initiative(),
        Some(("roof".to_string(), FeatureState::Active)),
        "the plan is moving even though no container is"
    );
}

/// The degradation criterion, stated exactly: a failure creating the
/// roof must cost neither the containers nor the issues.
#[tokio::test]
async fn a_roof_failure_does_not_cost_the_projects_or_the_issues() {
    let mut e = engine_with(&[("a-1", Some("Quayside"), TaskStatus::ToDo)]);
    let t = FakeTracker::new(PROVIDER);
    // Phase −1 runs first, so the next-call failure lands exactly on the roof.
    t.fail_next(TrackerError::Status {
        status: 500,
        body: "initiative service is down".into(),
    });

    let report = push(&mut e, &t, Some("roof")).await;

    assert!(
        report
            .initiative_failure
            .as_deref()
            .is_some_and(|why| why.contains("initiative service is down")),
        "the report says WHY the roof is missing: {report:?}"
    );
    assert!(report.initiative_ensured.is_none());
    assert_eq!(
        report.features_ensured,
        vec!["Quayside".to_string()],
        "the container still landed"
    );
    assert_eq!(t.items().len(), 1, "and so did the issue");
}

/// A provider without the concept degrades SILENTLY — the refusing default is
/// not a failure, exactly as `upsert_feature`'s flat-filing treatment.
#[tokio::test]
async fn a_provider_without_the_concept_is_not_reported_as_failing() {
    use async_trait::async_trait;
    use magistr_workflow::tracker::TrackerCapabilities;
    use magistr_workflow::tracker::domain::{WorkFeature, WorkItem};
    use magistr_workflow::tracker::port::{TrackerPort, UpsertOutcome};

    // Delegates everything it understands to the fake and deliberately does
    // NOT implement `upsert_initiative` — the port's refusing default IS the
    // behaviour under test.
    struct RoofBlind(FakeTracker);

    #[async_trait]
    impl TrackerPort for RoofBlind {
        fn provider(&self) -> &str {
            self.0.provider()
        }
        fn capabilities(&self) -> TrackerCapabilities {
            self.0.capabilities()
        }
        async fn upsert_item(&self, item: &WorkItem) -> Result<UpsertOutcome, TrackerError> {
            self.0.upsert_item(item).await
        }
        async fn fetch_since(&self, since: &str) -> Result<Vec<WorkItem>, TrackerError> {
            self.0.fetch_since(since).await
        }
        async fn upsert_feature(
            &self,
            feature: &WorkFeature,
        ) -> Result<UpsertOutcome, TrackerError> {
            self.0.upsert_feature(feature).await
        }
    }

    let mut e = engine_with(&[("a-1", Some("Quayside"), TaskStatus::ToDo)]);
    let t = RoofBlind(FakeTracker::new(PROVIDER));

    let report = project_all_with_policy(&mut e, &t, None, &Ownership::default(), Some("roof"))
        .await
        .expect("project");

    assert!(
        report.initiative_failure.is_none(),
        "Unsupported is a provider without the concept, not a failure"
    );
    assert!(report.initiative_ensured.is_none());
    assert_eq!(t.0.items().len(), 1, "the issue landed regardless");
}
