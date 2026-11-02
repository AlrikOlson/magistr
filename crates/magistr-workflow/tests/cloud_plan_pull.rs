//! Plan pull/reconcile: the cloud's plan tasks merge into
//! the local engine, making the local store a cache. NEWEST-wins per task
//! (reconcile-recency-guard): a stale cloud copy must never clobber a newer
//! local mutation — a race observed live, where a realtime
//! refresh raced a mutation's own fire-and-forget push and wiped a cross-ref.

use magistr_workflow::cloud::build::from_task;
use magistr_workflow::cloud::client::CloudClient;
use magistr_workflow::cloud::pull::reconcile_plan;
use magistr_workflow::plan::PlanEngine;
use magistr_workflow::plan::domain::{Task, TaskStatus};
use serde_json::{Value, json};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn task(id: &str, status: TaskStatus) -> Task {
    task_at(id, status, "2026-06-08T00:00:00Z")
}

/// A task that PROVABLY belongs to `project`. Ownership has to be stated in
/// the fixture now: the push no longer supplies it for an unstamped task, so a
/// test that wants "ours" must say so rather than letting the pusher's identity
/// stand in — which is the very laundering this suite guards against.
fn owned_task(id: &str, status: TaskStatus, project: &str) -> Task {
    Task {
        project_id: Some(project.into()),
        ..task(id, status)
    }
}

fn task_at(id: &str, status: TaskStatus, updated_at: &str) -> Task {
    Task {
        tier: None,
        id: id.into(),
        title: format!("Title {id}"),
        name: "node".into(),
        status,
        priority: 100,
        description: String::new(),
        content: None,
        feature: None,
        notes: String::new(),
        acceptance: vec![],
        deps: vec![],
        refs: vec![],
        shared: false,
        reprioritize: None,
        status_proposal: None,
        title_proposal: None,
        retired_reason: None,
        waiting_on: None,
        project_id: None,
        created_at: "2026-06-08T00:00:00Z".into(),
        updated_at: updated_at.into(),
    }
}

fn upsert(engine: &mut PlanEngine, mut task: Task) {
    task.project_id = Some("t".into());
    engine.upsert_task(task).unwrap();
}

#[test]
fn upsert_task_replaces_by_id_or_inserts() {
    let mut engine = PlanEngine::new("t".into());
    upsert(&mut engine, task("a", TaskStatus::Idea));
    upsert(&mut engine, task("b", TaskStatus::Idea));
    assert_eq!(engine.plan().tasks.len(), 2);

    // A NEWER cloud copy replaces 'a' — no duplicate, status updated.
    upsert(
        &mut engine,
        task_at("a", TaskStatus::Done, "2026-06-08T01:00:00Z"),
    );
    assert_eq!(engine.plan().tasks.len(), 2);
    let a = engine.plan().tasks.iter().find(|c| c.id == "a").unwrap();
    assert!(matches!(a.status, TaskStatus::Done));
}

/// THE OBSERVED RACE: a local mutation lands, then a refresh
/// delivers the cloud copy from BEFORE that mutation's push caught up. The
/// stale copy must not clobber the newer local state.
#[test]
fn upsert_task_keeps_a_newer_local_mutation_over_a_stale_cloud_copy() {
    let mut engine = PlanEngine::new("t".into());

    // The local task, freshly mutated (a cross-ref attached at T2).
    let mut local = task_at("a", TaskStatus::InProgress, "2026-06-08T02:00:00Z");
    local.refs = vec!["step:verify".into()];
    upsert(&mut engine, local);

    // The reconcile delivers the cloud copy from T1 — before the push landed.
    upsert(
        &mut engine,
        task_at("a", TaskStatus::InProgress, "2026-06-08T01:00:00Z"),
    );

    let a = engine.plan().tasks.iter().find(|c| c.id == "a").unwrap();
    assert_eq!(
        a.refs,
        vec!["step:verify".to_string()],
        "a stale cloud copy clobbered a newer local mutation"
    );
    assert_eq!(a.updated_at, "2026-06-08T02:00:00Z");
}

/// Equal stamps (the post-push echo of our own mutation) keep the local copy —
/// identical content, no churn. Same tie rule as the disk merge.
#[test]
fn upsert_task_keeps_local_on_equal_stamps() {
    let mut engine = PlanEngine::new("t".into());
    let mut local = task_at("a", TaskStatus::ToDo, "2026-06-08T01:00:00Z");
    local.notes = "local copy".into();
    upsert(&mut engine, local);

    upsert(
        &mut engine,
        task_at("a", TaskStatus::ToDo, "2026-06-08T01:00:00Z"),
    );
    assert_eq!(engine.plan().tasks[0].notes, "local copy");
}

#[tokio::test]
async fn reconcile_plan_pulls_cloud_tasks_into_the_local_engine() {
    let server = MockServer::start().await;
    let records: Vec<Value> = ["a", "b"]
        .iter()
        .map(|id| {
            serde_json::to_value(from_task(
                "t",
                &owned_task(id, TaskStatus::Idea, "t"),
                &[],
                &[],
            ))
            .unwrap()
        })
        .collect();
    Mock::given(method("GET"))
        .and(path("/v1/records"))
        .and(query_param("family", "plan"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "records": records })))
        .mount(&server)
        .await;

    let client = CloudClient::new(server.uri(), "tok");
    let mut engine = PlanEngine::new("t".into());

    let merged = reconcile_plan(&client, &mut engine).await.unwrap();
    assert_eq!(merged, 2);
    assert_eq!(engine.plan().tasks.len(), 2);

    // Re-pull acknowledges no new records.
    assert_eq!(reconcile_plan(&client, &mut engine).await.unwrap(), 0);
    assert_eq!(engine.plan().tasks.len(), 2);
}

/// sync-project-scope: with an org token every project in the org shares one
/// cloud tenant, so the reconcile must filter by the record's project stamp —
/// another project's tasks (or unstamped legacy records of unknowable
/// origin) must never land in this project's plan. This is the bleed
/// observed live between two projects sharing one tenant, as a test.
#[tokio::test]
async fn reconcile_skips_other_projects_records_in_the_same_tenant() {
    let server = MockServer::start().await;
    let mut unstamped =
        serde_json::to_value(from_task("t", &task("legacy", TaskStatus::Idea), &[], &[])).unwrap();
    unstamped["record"]
        .as_object_mut()
        .unwrap()
        .remove("project_id");
    let records: Vec<Value> = vec![
        serde_json::to_value(from_task(
            "t",
            &owned_task("ours", TaskStatus::Idea, "t"),
            &[],
            &[],
        ))
        .unwrap(),
        serde_json::to_value(from_task(
            "other-project",
            &task("theirs", TaskStatus::Idea),
            &[],
            &[],
        ))
        .unwrap(),
        unstamped,
    ];
    Mock::given(method("GET"))
        .and(path("/v1/records"))
        .and(query_param("family", "plan"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "records": records })))
        .mount(&server)
        .await;

    let client = CloudClient::new(server.uri(), "tok");
    let mut engine = PlanEngine::new("t".into());

    let merged = reconcile_plan(&client, &mut engine).await.unwrap();
    assert_eq!(merged, 1, "only this project's stamped task merges");
    let ids: Vec<&str> = engine.plan().tasks.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, vec!["ours"]);
}

/// THE LAUNDERING ROUND TRIP — the 2026-06 bleed's second act, closed.
///
/// A task of UNKNOWN origin (`project_id: None`) must not become provably the
/// pusher's just by being pushed. This is not a hypothetical: 22 tasks that had
/// bled into another project were retired there by a cleanup, and because sync
/// is write-through on mutation, that push stamped every one of them with the
/// cleaning project's id. They came back as provably its own — self-consistent,
/// and invisible to `prune`, which only removes what provably belongs elsewhere.
/// The cleanup is what made the bleed permanent.
///
/// So the property under test is a ROUND TRIP, not a field: build the envelope
/// the way a push does, feed it to the merge the way a pull does, and require
/// that it does NOT come home owned.
#[test]
fn a_push_cannot_launder_an_unprovable_task_into_ownership() {
    let orphan = task("bled-in-from-elsewhere", TaskStatus::Retired);
    assert!(
        orphan.project_id.is_none(),
        "precondition: origin genuinely unknown"
    );

    let envelope = serde_json::to_value(from_task("project-c59cfc", &orphan, &[], &[])).unwrap();
    assert_eq!(
        envelope["record"].get("project_id"),
        Some(&Value::Null),
        "an unprovable origin must cross the wire as explicitly unprovable, not \
         be filled in with whoever happened to push it"
    );

    // The half that actually protects anyone: it must not merge back as ours.
    let mut engine = PlanEngine::new("project-c59cfc".into());
    let merged =
        magistr_workflow::cloud::pull::apply_plan_records(&mut engine, &[envelope]).unwrap();
    assert_eq!(
        merged, 0,
        "the pusher got its own unprovable record back as provably its own — this \
         is the laundering that turned a cleanup into permanent ownership"
    );
    assert!(
        engine.plan().tasks.is_empty(),
        "nothing of unknown origin may land in a project's store by round trip"
    );
}

/// The other half of the contract: a task that DOES know where it came from is
/// stamped, merges, and is not relabelled by whoever pushes it.
#[test]
fn a_provable_task_still_round_trips_and_keeps_its_own_origin() {
    let ours = owned_task("genuinely-ours", TaskStatus::Idea, "project-c59cfc");
    let envelope = serde_json::to_value(from_task("project-c59cfc", &ours, &[], &[])).unwrap();
    assert_eq!(
        envelope["record"]["project_id"].as_str(),
        Some("project-c59cfc")
    );

    let mut engine = PlanEngine::new("project-c59cfc".into());
    let merged =
        magistr_workflow::cloud::pull::apply_plan_records(&mut engine, &[envelope]).unwrap();
    assert_eq!(merged, 1, "a provable record must still sync");

    // And a foreign one, pushed by its own project, stays foreign to us.
    let theirs = owned_task("theirs", TaskStatus::Idea, "magistr-676f38");
    let foreign = serde_json::to_value(from_task("magistr-676f38", &theirs, &[], &[])).unwrap();
    let merged =
        magistr_workflow::cloud::pull::apply_plan_records(&mut engine, &[foreign]).unwrap();
    assert_eq!(merged, 0, "another project's stamped record never merges");
}
