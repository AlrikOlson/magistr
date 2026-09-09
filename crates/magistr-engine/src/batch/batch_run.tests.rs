use super::*;
use crate::batch::project_dep::ProjectDep;
use crate::fleet::{FleetNode, FleetRegistry};
use chrono::{TimeZone, Utc};

fn at(secs: i64) -> chrono::DateTime<chrono::Utc> {
    Utc.timestamp_opt(1_700_000_000 + secs, 0)
        .single()
        .expect("a valid instant")
}

fn plan(projects: Vec<ProjectDep>) -> BatchRun {
    let graph = ProjectGraph::new(projects).expect("a well-formed graph");
    BatchRun::plan("b1", "ship the next chunk", graph).expect("an orderable batch")
}

/// A node that serves every project, with room for the whole batch.
fn fleet_with_room(capacity: u32) -> FleetRegistry {
    let mut fleet = FleetRegistry::new();
    fleet.register(FleetNode::new("n1", "host", "1.0", at(0)).with_capacity(capacity));
    fleet
}

/// Run a project's job all the way through to an outcome, claiming it
/// first when nothing has yet.
fn settle(queue: &mut WorkQueue, fleet: &FleetRegistry, job_id: &str, succeeded: bool) {
    let held = queue
        .job(job_id)
        .filter(|job| job.state == crate::fleet::JobState::Claimed)
        .map(|job| job.attempt);
    let attempt = match held {
        Some(attempt) => attempt,
        None => queue
            .claim(fleet, job_id, "n1", at(1))
            .expect("the node can take it"),
    };
    queue
        .complete(job_id, "n1", attempt, succeeded)
        .expect("the holder reports");
}

// ── Ordering and cycles ─────────────────────────────────────────────

#[test]
fn a_batch_is_planned_in_dependency_order() {
    let run = plan(vec![
        ProjectDep::new("web").after(["api"]),
        ProjectDep::new("api").after(["types"]),
        ProjectDep::new("types"),
    ]);
    assert_eq!(run.order(), ["types", "api", "web"]);
}

#[test]
fn a_cyclic_batch_is_refused_at_planning_and_never_becomes_a_run() {
    let graph = ProjectGraph::new(vec![
        ProjectDep::new("a").after(["b"]),
        ProjectDep::new("b").after(["a"]),
    ])
    .expect("a graph");
    let err = BatchRun::plan("b1", "go", graph).expect_err("a cycle cannot be planned");
    assert!(err.to_string().contains("cycle"), "{err}");
}

#[test]
fn a_job_id_is_derived_from_the_batch_and_the_project() {
    let run = plan(vec![ProjectDep::new("api")]);
    assert_eq!(run.job_id("api"), "b1:api");
    assert_eq!(run.id(), "b1");
    assert_eq!(run.prompt(), "ship the next chunk");
}

// ── Release waves ───────────────────────────────────────────────────

#[test]
fn only_the_roots_are_released_first_and_the_rest_wait() {
    let mut queue = WorkQueue::new();
    let mut run = plan(vec![
        ProjectDep::new("web").after(["api"]),
        ProjectDep::new("api"),
        ProjectDep::new("docs"),
    ]);

    let released = run.advance(&mut queue).expect("first wave");
    assert_eq!(released, ["api", "docs"], "both roots, in order");
    assert_eq!(run.state_of(&queue, "api"), BatchState::Queued);
    assert_eq!(run.state_of(&queue, "web"), BatchState::Waiting);
    assert_eq!(queue.len(), 2, "nothing was submitted for web");
}

#[test]
fn a_released_job_carries_the_batch_prompt_project_and_requester() {
    let mut queue = WorkQueue::new();
    let graph = ProjectGraph::new(vec![ProjectDep::new("api")]).expect("a graph");
    let mut run = BatchRun::plan("b1", "go", graph)
        .expect("planned")
        .requested_by("client-7");

    run.advance(&mut queue).expect("released");
    let job = queue.job("b1:api").expect("submitted");
    assert_eq!(job.unit.project, "api");
    assert_eq!(job.unit.prompt, "go");
    assert_eq!(job.unit.requested_by, "client-7");
}

#[test]
fn a_dependent_is_released_only_once_its_dependency_succeeded() {
    let fleet = fleet_with_room(4);
    let mut queue = WorkQueue::new();
    let mut run = plan(vec![
        ProjectDep::new("web").after(["api"]),
        ProjectDep::new("api"),
    ]);

    run.advance(&mut queue).expect("first wave");
    assert!(
        run.advance(&mut queue).expect("second call").is_empty(),
        "nothing has finished, so nothing new is released"
    );

    // Claimed is not enough — the dependency must actually succeed.
    let attempt = queue.claim(&fleet, "b1:api", "n1", at(1)).expect("claimed");
    assert_eq!(run.state_of(&queue, "api"), BatchState::Running);
    assert!(run.advance(&mut queue).expect("still running").is_empty());

    queue
        .complete("b1:api", "n1", attempt, true)
        .expect("reported");
    assert_eq!(run.advance(&mut queue).expect("released"), ["web"]);
    assert_eq!(run.state_of(&queue, "web"), BatchState::Queued);
}

#[test]
fn advancing_a_finished_batch_releases_nothing_and_settles() {
    let fleet = fleet_with_room(4);
    let mut queue = WorkQueue::new();
    let mut run = plan(vec![ProjectDep::new("api")]);

    run.advance(&mut queue).expect("released");
    settle(&mut queue, &fleet, "b1:api", true);

    assert!(run.advance(&mut queue).expect("nothing left").is_empty());
    assert!(run.is_settled(&queue));
    assert_eq!(run.state_of(&queue, "api"), BatchState::Succeeded);
}

// ── A failure stops its dependents, not the batch ───────────────────

#[test]
fn a_failure_skips_its_transitive_dependents_and_leaves_the_rest_running() {
    let fleet = fleet_with_room(8);
    let mut queue = WorkQueue::new();
    let mut run = plan(vec![
        ProjectDep::new("types"),
        ProjectDep::new("api").after(["types"]),
        ProjectDep::new("web").after(["api"]),
        ProjectDep::new("docs"),
    ]);

    run.advance(&mut queue).expect("types and docs");
    settle(&mut queue, &fleet, "b1:types", false);

    let released = run.advance(&mut queue).expect("after the failure");
    assert!(released.is_empty(), "nothing downstream may start");
    assert_eq!(run.state_of(&queue, "types"), BatchState::Failed);
    assert_eq!(run.state_of(&queue, "api"), BatchState::Skipped, "direct");
    assert_eq!(
        run.state_of(&queue, "web"),
        BatchState::Skipped,
        "transitive"
    );
    assert_eq!(
        run.state_of(&queue, "docs"),
        BatchState::Queued,
        "an independent branch is untouched by the failure"
    );

    settle(&mut queue, &fleet, "b1:docs", true);
    assert!(
        run.is_settled(&queue),
        "the batch finishes without its lost leg"
    );
}

#[test]
fn a_skipped_project_is_never_submitted_to_the_queue() {
    let fleet = fleet_with_room(4);
    let mut queue = WorkQueue::new();
    let mut run = plan(vec![
        ProjectDep::new("api"),
        ProjectDep::new("web").after(["api"]),
    ]);

    run.advance(&mut queue).expect("api");
    settle(&mut queue, &fleet, "b1:api", false);
    run.advance(&mut queue).expect("cascade");

    assert!(queue.job("b1:web").is_none(), "no job was ever created");
    assert_eq!(queue.len(), 1);
}

#[test]
fn a_failure_downstream_of_a_failure_does_not_unskip_anything() {
    // Advancing repeatedly must be stable: the cascade is idempotent.
    let fleet = fleet_with_room(4);
    let mut queue = WorkQueue::new();
    let mut run = plan(vec![
        ProjectDep::new("api"),
        ProjectDep::new("web").after(["api"]),
    ]);

    run.advance(&mut queue).expect("api");
    settle(&mut queue, &fleet, "b1:api", false);
    for _ in 0..3 {
        assert!(run.advance(&mut queue).expect("stable").is_empty());
        assert_eq!(run.state_of(&queue, "web"), BatchState::Skipped);
    }
}

#[test]
fn a_running_project_is_never_skipped_by_an_unrelated_failure() {
    let fleet = fleet_with_room(8);
    let mut queue = WorkQueue::new();
    let mut run = plan(vec![ProjectDep::new("api"), ProjectDep::new("docs")]);

    run.advance(&mut queue).expect("both roots");
    queue
        .claim(&fleet, "b1:docs", "n1", at(1))
        .expect("claimed");
    settle(&mut queue, &fleet, "b1:api", false);

    run.advance(&mut queue).expect("cascade");
    assert_eq!(
        run.state_of(&queue, "docs"),
        BatchState::Running,
        "the failure is not upstream of docs, so docs keeps running"
    );
}

// ── Monorepo: one working tree, one project at a time ───────────────

#[test]
fn two_archetypes_under_one_tree_are_released_one_at_a_time() {
    let fleet = fleet_with_room(8);
    let mut queue = WorkQueue::new();
    let mut run = plan(vec![
        ProjectDep::new("mono-api").in_worktree("/repo"),
        ProjectDep::new("mono-web").in_worktree("/repo"),
        ProjectDep::new("other").in_worktree("/elsewhere"),
    ]);

    let first = run.advance(&mut queue).expect("first wave");
    assert_eq!(
        first,
        ["mono-api", "other"],
        "one project per tree — never both archetypes of /repo"
    );
    assert_eq!(run.state_of(&queue, "mono-web"), BatchState::Waiting);

    // Still held while its sibling is merely queued: the job is out
    // there and a node may take it at any moment.
    assert!(run.advance(&mut queue).expect("sibling queued").is_empty());
    queue
        .claim(&fleet, "b1:mono-api", "n1", at(1))
        .expect("claimed");
    assert!(run.advance(&mut queue).expect("sibling running").is_empty());

    settle(&mut queue, &fleet, "b1:mono-api", true);
    assert_eq!(
        run.advance(&mut queue).expect("the tree is free"),
        ["mono-web"]
    );
}

#[test]
fn a_shared_tree_is_freed_by_a_failure_too_and_the_sibling_still_runs() {
    // The exclusion is on the tree, not on success: a sibling that does
    // not depend on the failed project has no reason to be skipped.
    let fleet = fleet_with_room(8);
    let mut queue = WorkQueue::new();
    let mut run = plan(vec![
        ProjectDep::new("mono-api").in_worktree("/repo"),
        ProjectDep::new("mono-web").in_worktree("/repo"),
    ]);

    run.advance(&mut queue).expect("first");
    settle(&mut queue, &fleet, "b1:mono-api", false);

    assert_eq!(run.advance(&mut queue).expect("second"), ["mono-web"]);
    assert_eq!(run.state_of(&queue, "mono-api"), BatchState::Failed);
}

#[test]
fn projects_without_a_declared_tree_run_concurrently() {
    let mut queue = WorkQueue::new();
    let mut run = plan(vec![
        ProjectDep::new("a"),
        ProjectDep::new("b"),
        ProjectDep::new("c"),
    ]);
    assert_eq!(
        run.advance(&mut queue).expect("all three"),
        ["a", "b", "c"],
        "an undeclared tree serializes nothing"
    );
}

#[test]
fn the_tree_and_the_dependency_rule_compose() {
    // web depends on api and both live in /repo: the dependency is the
    // binding constraint, and the tree never releases them together.
    let fleet = fleet_with_room(8);
    let mut queue = WorkQueue::new();
    let mut run = plan(vec![
        ProjectDep::new("api").in_worktree("/repo"),
        ProjectDep::new("web").after(["api"]).in_worktree("/repo"),
    ]);

    assert_eq!(run.advance(&mut queue).expect("first"), ["api"]);
    settle(&mut queue, &fleet, "b1:api", true);
    assert_eq!(run.advance(&mut queue).expect("second"), ["web"]);
}

// ── The rollup ──────────────────────────────────────────────────────

#[test]
fn the_rollup_reports_the_whole_run_in_dependency_order() {
    let fleet = fleet_with_room(8);
    let mut queue = WorkQueue::new();
    let mut run = plan(vec![
        ProjectDep::new("types"),
        ProjectDep::new("api").after(["types"]),
        ProjectDep::new("web").after(["api"]),
        ProjectDep::new("docs"),
    ]);

    run.advance(&mut queue).expect("first wave: docs and types");
    settle(&mut queue, &fleet, "b1:types", true);
    run.advance(&mut queue).expect("second wave: api");
    queue.claim(&fleet, "b1:api", "n1", at(2)).expect("claimed");

    let status = run.status(&queue);
    assert_eq!(status.batch_id, "b1");
    assert_eq!(status.total, 4);
    assert_eq!(status.succeeded, 1, "types");
    assert_eq!(status.running, 1, "api");
    assert_eq!(status.queued, 1, "docs");
    assert_eq!(status.waiting, 1, "web");
    assert!(!status.settled);
    assert_eq!(
        status
            .rows
            .iter()
            .map(|row| row.project.as_str())
            .collect::<Vec<_>>(),
        ["docs", "types", "api", "web"],
        "rows follow the planned order — ready projects by id, then their dependents"
    );
    assert_eq!(status.rows[0].job_id, "b1:docs");
}

#[test]
fn an_unadvanced_batch_reports_every_project_waiting() {
    let queue = WorkQueue::new();
    let run = plan(vec![
        ProjectDep::new("api"),
        ProjectDep::new("web").after(["api"]),
    ]);
    let status = run.status(&queue);
    assert_eq!(status.waiting, 2);
    assert!(!status.settled, "a batch that has not started is not done");
}

#[test]
fn an_empty_batch_is_settled_from_the_start() {
    let mut queue = WorkQueue::new();
    let mut run = plan(vec![]);
    assert!(run.advance(&mut queue).expect("nothing").is_empty());
    assert!(run.is_settled(&queue));
    assert_eq!(run.status(&queue).total, 0);
}

// ── Refusals ────────────────────────────────────────────────────────

#[test]
fn two_batches_sharing_an_id_collide_on_the_queue_rather_than_double_submitting() {
    let mut queue = WorkQueue::new();
    let mut first = plan(vec![ProjectDep::new("api")]);
    first.advance(&mut queue).expect("released");

    let mut second = plan(vec![ProjectDep::new("api")]);
    let err = second
        .advance(&mut queue)
        .expect_err("the job id is already taken");
    assert!(err.to_string().contains("already been submitted"), "{err}");
}

#[test]
fn a_forgotten_job_reads_as_queued_rather_than_vanishing() {
    // An operator retiring a job out from under a live batch is the one
    // case the queue cannot answer for. The run knows it asked and will
    // not ask twice, so "waiting to be picked up" is the honest reading.
    let mut queue = WorkQueue::new();
    let mut run = plan(vec![ProjectDep::new("api")]);
    run.advance(&mut queue).expect("released");

    assert!(queue.forget("b1:api"));
    assert_eq!(run.state_of(&queue, "api"), BatchState::Queued);
    assert!(run.advance(&mut queue).expect("no resubmit").is_empty());
}
