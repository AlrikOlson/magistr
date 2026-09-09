//! Integration tests for fleet scheduling, session migration, and policy.
//!
//! The CLI owns the roster and directory configuration; `magistr-engine`
//! owns the fleet, queue, snapshots, and batches. These tests exercise their
//! shared data and check that placement, capacity, and reporting rights use
//! the current lease and attempt state.
//!
//! Nodes are simulated by calls to the control-plane APIs. These tests do
//! not run agents or verify a node-side executor. Policy bundles are built
//! from team configuration in the fixture; publication is also simulated.

use chrono::{DateTime, Duration, TimeZone, Utc};

use magistr::team::directory::{
    DirectoryConfig, DirectoryOutcome, GroupRole, ScimRequest, ScimUser, ScimValue, apply_scim,
    resolve_role_with_directory,
};
use magistr::team::{Identity, TeamConfig, TeamMember, TeamRole};

use magistr_engine::batch::{BatchRun, BatchState, ProjectDep, ProjectGraph};
use magistr_engine::cost::{BudgetPeriod, BudgetRule, BudgetScope};
use magistr_engine::fleet::{
    FleetNode, FleetRegistry, JobState, NodeHealth, PolicyBundle, WorkQueue, WorkUnit,
};
use magistr_engine::policy::{GovernancePolicy, PolicyEffect, PolicyRequest};
use magistr_engine::session::{
    EngineSession, SessionEvent, SessionResidency, SessionSnapshot, SessionStatus,
};
use magistr_engine::workflow::{IterationState, IterationStatus, WorkflowPhase};

// ── Fixture ─────────────────────────────────────────────────────────

const API: &str = "acme-api";
const WEB: &str = "acme-web";
const CORE: &str = "acme-core";

const ALICE: &str = "alice@acme.example";
const BOB: &str = "bob@acme.example";

/// The fleet's lease TTL, kept short so a test can walk a node past it
/// in whole minutes rather than by mocking a clock.
const LEASE_TTL: i64 = 60;

fn at(h: u32, m: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 8, 30, h, m, 0)
        .single()
        .expect("valid instant")
}

/// A registry holding one node per (id, projects, capacity) triple,
/// every lease renewed at `now`.
fn fleet_of(now: DateTime<Utc>, nodes: &[(&str, &[&str], u32)]) -> FleetRegistry {
    let mut fleet = FleetRegistry::with_lease_ttl_secs(LEASE_TTL);
    for (id, projects, capacity) in nodes {
        let node = FleetNode::new(*id, format!("{id}.acme.example"), "0.1.0", now)
            .with_projects(projects.iter().map(|p| (*p).to_string()).collect())
            .with_capacity(*capacity);
        fleet.register(node);
    }
    fleet
}

fn unit(id: &str, project: &str) -> WorkUnit {
    WorkUnit::new(id, project, "advance the roadmap").requested_by(ALICE)
}

fn identity(id: &str) -> Identity {
    Identity {
        name: id.to_string(),
        email: Some(id.to_string()),
        provider: None,
    }
}

/// The org's guardrails: a tool the fleet denies everywhere.
fn org_policy() -> GovernancePolicy {
    GovernancePolicy {
        tool_denylist: vec!["Bash".into()],
        required_gates: vec!["test".into()],
        ..GovernancePolicy::default()
    }
}

fn org_budgets() -> Vec<BudgetRule> {
    vec![BudgetRule::new(BudgetScope::Org, BudgetPeriod::Monthly).limit(2_000.0)]
}

/// A session that has got somewhere, so a snapshot of it carries state
/// worth migrating rather than a default.
fn working_session(id: &str) -> EngineSession {
    let mut session = EngineSession::new(id);
    session
        .set_status(SessionStatus::InProgress)
        .expect("created -> in progress");
    session.set_phase(WorkflowPhase::Executing);
    session.record_turn(1.25, 400, 90);
    session.push_message("implement the fleet join");
    session
}

fn iteration(n: u32) -> IterationState {
    IterationState {
        iteration: n,
        max_iterations: 20,
        last_status: IterationStatus::Committed,
        spec_checked: 7,
        spec_total: 12,
    }
}

fn snapshot_of(session: &EngineSession, n: u32, when: DateTime<Utc>) -> SessionSnapshot {
    SessionSnapshot::capture(session, iteration(n), when)
}

fn phase_event(to: WorkflowPhase) -> SessionEvent {
    SessionEvent::PhaseChanged {
        from: WorkflowPhase::Planning,
        to,
    }
}

// Expired node leases stop placement and release held work.
// The queue consults the fleet directly instead of caching node health.

#[test]
fn the_lease_that_decides_health_is_the_lease_that_decides_placement() {
    let start = at(9, 0);
    let mut fleet = fleet_of(start, &[("node-a", &[API], 1)]);
    let mut queue = WorkQueue::new();
    queue.submit(unit("job-1", API)).expect("submit");

    // Ready: the queue places on it.
    assert_eq!(
        fleet.node("node-a").map(|n| n.health(start, LEASE_TTL)),
        Some(NodeHealth::Ready)
    );
    assert_eq!(
        queue.place(&fleet, start),
        Some(("job-1".into(), "node-a".into())),
        "a Ready node that serves the project takes the queued job"
    );

    // Now the node goes quiet. Nothing is told: no state is written, no
    // callback fires, no queue field is updated. The *only* thing that
    // changed is the instant the question is asked at.
    let gone = start + Duration::seconds(LEASE_TTL * 4);
    assert_eq!(
        fleet.node("node-a").map(|n| n.health(gone, LEASE_TTL)),
        Some(NodeHealth::Unreachable)
    );
    assert_eq!(
        queue.place(&fleet, gone),
        None,
        "the queue reads health at `now` rather than caching it, so a \
         node that stopped renewing stops being placed on without \
         anything having to notify the queue"
    );

    // And it is reversible on the same terms: one heartbeat and the
    // node is placeable again, with no queue-side reset.
    fleet
        .heartbeat("node-a", gone, 0, "")
        .expect("registered node renews");
    assert_eq!(
        queue.place(&fleet, gone),
        Some(("job-1".into(), "node-a".into())),
        "a renewed lease restores placement with no queue-side state to \
         clear"
    );
}

#[test]
fn a_node_that_dies_holding_work_has_it_reclaimed_and_placed_elsewhere() {
    let start = at(9, 0);
    let mut fleet = fleet_of(start, &[("node-a", &[API], 1), ("node-b", &[API], 1)]);
    let mut queue = WorkQueue::new();
    queue.submit(unit("job-1", API)).expect("submit");

    let attempt = queue
        .claim(&fleet, "job-1", "node-a", start)
        .expect("claim on a Ready node");
    assert_eq!(attempt, 1);
    assert_eq!(queue.in_flight("node-a"), 1);
    assert_eq!(
        queue.free_slots(&fleet, "node-a", start),
        0,
        "capacity is counted from the jobs the node holds, not tallied"
    );

    // node-a dies. node-b keeps renewing, so this is a *node* failure
    // and not a clock skew the whole fleet shares.
    let later = start + Duration::seconds(LEASE_TTL * 4);
    fleet
        .heartbeat("node-b", later, 0, "")
        .expect("node-b is alive");

    let reclaimed = queue.reclaim(&fleet, later);
    assert_eq!(reclaimed, vec!["job-1".to_string()]);
    assert_eq!(queue.job("job-1").map(|j| j.state), Some(JobState::Queued));
    assert_eq!(
        queue.in_flight("node-a"),
        0,
        "releasing the claim releases the capacity, because the capacity \
         *is* the claim count"
    );

    // The work is not lost — it lands on the surviving node, and the
    // attempt number is what the new holder must present.
    let (job, node) = queue
        .place(&fleet, later)
        .expect("the surviving node takes the freed job");
    assert_eq!((job.as_str(), node.as_str()), ("job-1", "node-b"));
    let second = queue
        .claim(&fleet, "job-1", "node-b", later)
        .expect("claim by the survivor");
    assert_eq!(
        second, 2,
        "the attempt is bumped by the claim, so it is both the retry \
         count and the fence"
    );
}

#[test]
fn a_job_no_registered_node_serves_waits_without_blocking_the_queue() {
    let now = at(9, 0);
    let fleet = fleet_of(now, &[("node-a", &[API], 2)]);
    let mut queue = WorkQueue::new();
    // Submitted first, so it is at the head of the order.
    queue.submit(unit("job-orphan", WEB)).expect("submit");
    queue.submit(unit("job-1", API)).expect("submit");

    let placed = queue.dispatch(&fleet, now);
    assert_eq!(
        placed,
        vec![("job-1".to_string(), "node-a".to_string())],
        "the job no node serves is skipped rather than holding the head \
         of the queue against the job that can run"
    );
    assert_eq!(
        queue.job("job-orphan").map(|j| j.state),
        Some(JobState::Queued),
        "and it is still queued, waiting for a node that serves it"
    );
}

// Maintenance migration: cordon a node, drain its queue, and restore the
// last checkpoint on another node. Cordoning is explicit because a healthy
// node can still need maintenance; capacity and placement derive from it.

#[test]
fn cordoning_a_healthy_node_collapses_its_capacity_without_touching_its_health() {
    let now = at(9, 0);
    let mut fleet = fleet_of(now, &[("node-a", &[API], 4)]);
    let queue = WorkQueue::new();

    assert_eq!(queue.free_slots(&fleet, "node-a", now), 4);

    fleet.cordon("node-a", true).expect("cordon a known node");

    assert_eq!(
        fleet.node("node-a").map(|n| n.health(now, LEASE_TTL)),
        Some(NodeHealth::Ready),
        "a cordoned node is still perfectly healthy — that is why \
         health could never have expressed the cordon"
    );
    assert_eq!(
        queue.free_slots(&fleet, "node-a", now),
        0,
        "but it has no free slots, which is what makes the cordon real \
         to the scheduler"
    );
    assert_eq!(queue.place(&fleet, now), None);
}

#[test]
fn draining_refuses_a_node_that_was_never_cordoned() {
    let now = at(9, 0);
    let fleet = fleet_of(now, &[("node-a", &[API], 1)]);
    let mut queue = WorkQueue::new();
    queue.submit(unit("job-1", API)).expect("submit");
    queue.claim(&fleet, "job-1", "node-a", now).expect("claim");

    let refused = queue
        .drain(&fleet, "node-a")
        .expect_err("an uncordoned node cannot be drained");
    assert!(
        refused.to_string().contains("cordoned"),
        "the refusal says why: evicting before cordoning would place the \
         freed work straight back on the machine it came from — {refused}"
    );
    assert_eq!(
        queue.job("job-1").map(|j| j.state),
        Some(JobState::Claimed),
        "and the refusal is total: the job is untouched"
    );
}

#[test]
fn a_drained_job_carries_its_checkpoint_to_the_node_that_takes_it_next() {
    let now = at(9, 0);
    let mut fleet = fleet_of(now, &[("node-a", &[API], 1), ("node-b", &[API], 1)]);
    let mut queue = WorkQueue::new();
    queue.submit(unit("job-1", API)).expect("submit");

    // node-a claims it and gets some way through a session.
    let attempt = queue.claim(&fleet, "job-1", "node-a", now).expect("claim");
    let session = working_session("sess-migrate");
    queue
        .checkpoint("job-1", "node-a", attempt, snapshot_of(&session, 3, now))
        .expect("the holder may checkpoint");

    // The operator cordons node-a for maintenance and drains it.
    fleet.cordon("node-a", true).expect("cordon");
    let handed_off = queue
        .drain(&fleet, "node-a")
        .expect("drain a cordoned node");
    assert_eq!(handed_off, vec!["job-1".to_string()]);
    assert_eq!(
        queue.job("job-1").map(|j| j.state),
        Some(JobState::Queued),
        "a drained job is Queued, never Failed — it did not fail, its \
         machine went away"
    );

    // The cordon is what keeps the freed work off node-a: the only
    // placement is the other node.
    let (job, node) = queue.place(&fleet, now).expect("placeable elsewhere");
    assert_eq!((job.as_str(), node.as_str()), ("job-1", "node-b"));

    // And what node-a checkpointed is what node-b resumes from — the
    // snapshot is a fact about the *job*, so it survives the requeue and
    // the new claim.
    queue.claim(&fleet, "job-1", "node-b", now).expect("claim");
    let resume_from = queue
        .job("job-1")
        .and_then(|j| j.snapshot.as_ref())
        .expect("the checkpoint survived the hand-off");
    assert_eq!(resume_from.session_id, "sess-migrate");
    assert_eq!(resume_from.iteration.iteration, 3);

    // Restoring it on the new machine is the cross-machine resume.
    let restored = resume_from.restore().expect("restorable");
    assert_eq!(restored.iteration.iteration, 3);
    assert_eq!(
        restored.session.status(),
        SessionStatus::Awaiting,
        "a status that asserts something about a live process cannot \
         survive the move: InProgress lands at the turn boundary"
    );
    assert_eq!(
        restored.resumed_from,
        Some(SessionStatus::InProgress),
        "and where it came from is on the record rather than erased"
    );
}

#[test]
fn a_session_holding_an_unanswered_permission_refuses_to_be_drained_onward() {
    let now = at(9, 0);
    let mut fleet = fleet_of(now, &[("node-a", &[API], 1), ("node-b", &[API], 1)]);
    let mut queue = WorkQueue::new();
    queue.submit(unit("job-1", API)).expect("submit");
    let attempt = queue.claim(&fleet, "job-1", "node-a", now).expect("claim");

    let session = working_session("sess-blocked");
    let blocked = snapshot_of(&session, 2, now)
        .holding(SessionResidency::AgentProcess { pid: Some(4242) })
        .holding(SessionResidency::PendingPermission {
            request_id: "req-7".into(),
        });
    queue
        .checkpoint("job-1", "node-a", attempt, blocked)
        .expect("checkpoint");

    // The drain itself still works — the control plane's job is to free
    // the machine, and it does.
    fleet.cordon("node-a", true).expect("cordon");
    assert_eq!(
        queue.drain(&fleet, "node-a").expect("drain"),
        vec!["job-1".to_string()]
    );

    // What refuses is the *restore*, on the new host, and it refuses
    // whole rather than resuming a session that believes a question is
    // outstanding which can never now be answered.
    let carried = queue
        .job("job-1")
        .and_then(|j| j.snapshot.as_ref())
        .expect("snapshot carried");
    let refused = carried
        .restore()
        .expect_err("a blocking residency refuses the whole restore");
    assert!(
        refused.to_string().contains("req-7"),
        "and it names what blocked it — {refused}"
    );
}

#[test]
fn what_a_migrated_session_lost_is_listed_rather_than_silently_dropped() {
    let now = at(9, 0);
    let session = working_session("sess-sever");
    let snapshot = snapshot_of(&session, 5, now)
        .resident_on(vec![
            SessionResidency::AgentProcess { pid: Some(99) },
            SessionResidency::Terminal {
                id: "term-1".into(),
            },
            SessionResidency::WorkbenchLock {
                workspace: "/srv/acme-api".into(),
            },
        ])
        .with_no_commits(2);

    let restored = snapshot.restore().expect("every residency is severable");

    assert!(restored.lost_anything());
    assert_eq!(
        restored.severed.len(),
        3,
        "all three severable residencies come back on the record"
    );
    assert_eq!(
        restored.consecutive_no_commits, 2,
        "the no-commit counter crosses the machine, because it is a stop \
         condition rather than a statistic — dropping it would let a \
         wedged loop launder its own stop by moving machines"
    );
    let report = restored.severance_report();
    assert_eq!(report.len(), 3);
    assert!(report.iter().any(|line| line.contains("term-1")));
    assert!(report.iter().any(|line| line.contains("/srv/acme-api")));
}

// ── The fence: an evicted node cannot write over its successor ──────

#[test]
fn the_attempt_fence_holds_at_every_verb_a_node_can_call() {
    let now = at(9, 0);
    let mut fleet = fleet_of(now, &[("node-a", &[API], 1), ("node-b", &[API], 1)]);
    let mut queue = WorkQueue::new();
    queue.submit(unit("job-1", API)).expect("submit");

    let first = queue.claim(&fleet, "job-1", "node-a", now).expect("claim");
    let session = working_session("sess-fence");
    queue
        .checkpoint("job-1", "node-a", first, snapshot_of(&session, 3, now))
        .expect("the holder checkpoints at iteration 3");

    // node-a is drained; node-b picks the job up and moves it on.
    fleet.cordon("node-a", true).expect("cordon");
    queue.drain(&fleet, "node-a").expect("drain");
    let second = queue.claim(&fleet, "job-1", "node-b", now).expect("claim");
    assert_eq!(second, first + 1);
    queue
        .checkpoint("job-1", "node-b", second, snapshot_of(&session, 9, now))
        .expect("the new holder checkpoints at iteration 9");

    // Now node-a wakes up — a paused process, exactly the case a lease
    // alone cannot cover — and tries all three of its verbs at the
    // attempt it still believes it holds.
    queue
        .report(
            "job-1",
            "node-a",
            first,
            0,
            phase_event(WorkflowPhase::Verifying),
        )
        .expect_err("a stale holder cannot report events");
    queue
        .checkpoint("job-1", "node-a", first, snapshot_of(&session, 4, now))
        .expect_err("a stale holder cannot overwrite the state its successor works from");
    queue
        .complete("job-1", "node-a", first, true)
        .expect_err("a stale holder's late success is refused, not double-counted");

    // The successor's state is intact and it can still finish.
    assert_eq!(
        queue
            .job("job-1")
            .and_then(|j| j.snapshot.as_ref())
            .map(|s| s.iteration.iteration),
        Some(9),
        "the fenced-out node's checkpoint never landed"
    );
    assert_eq!(
        queue
            .complete("job-1", "node-b", second, true)
            .expect("the real holder completes"),
        JobState::Completed
    );
}

#[test]
fn a_job_that_runs_twice_appends_to_the_readers_stream_rather_than_rewinding_it() {
    let now = at(9, 0);
    let mut fleet = fleet_of(now, &[("node-a", &[API], 1), ("node-b", &[API], 1)]);
    let mut queue = WorkQueue::new();
    queue.submit(unit("job-1", API)).expect("submit");

    // Attempt 1 on node-a delivers two events, out of order on the wire.
    let first = queue.claim(&fleet, "job-1", "node-a", now).expect("claim");
    queue
        .report(
            "job-1",
            "node-a",
            first,
            1,
            phase_event(WorkflowPhase::Executing),
        )
        .expect("seq 1 buffers behind the gap");
    queue
        .report(
            "job-1",
            "node-a",
            first,
            0,
            phase_event(WorkflowPhase::Planning),
        )
        .expect("seq 0 arrives and drains the buffer");

    let seen = queue.events("job-1", 0).expect("a live job has a relay");
    assert_eq!(seen.len(), 2, "an out-of-order transport delivers in order");
    let cursor = seen.len();

    // node-a dies mid-job and node-b re-runs it, numbering its own
    // events from zero again.
    let later = now + Duration::seconds(LEASE_TTL * 4);
    fleet
        .heartbeat("node-b", later, 0, "")
        .expect("node-b alive");
    assert_eq!(queue.reclaim(&fleet, later), vec!["job-1".to_string()]);
    let second = queue
        .claim(&fleet, "job-1", "node-b", later)
        .expect("re-claim");
    queue
        .report(
            "job-1",
            "node-b",
            second,
            0,
            phase_event(WorkflowPhase::Verifying),
        )
        .expect("the new attempt restarts the producer's sequence");

    // The reader's cursor still means what it meant: it is an index into
    // an append-only log, not into the producer's sequence.
    assert_eq!(
        queue.events("job-1", cursor).map(<[_]>::len),
        Some(1),
        "the re-run appends one event past where the reader had got to \
         rather than replaying from zero"
    );
    assert_eq!(
        queue.events("job-1", 0).map(<[_]>::len),
        Some(3),
        "and nothing already handed out was truncated"
    );
}

// ── Criterion 4: three schedulers stacked, each deriving from the one
//    below ─────────────────────────────────────────────────────────
//
// `BatchRun` consults no clock and never sees the fleet: it schedules
// *projects* into the queue, and the queue schedules *machines*. The
// join to assert is that the stack composes — a batch's own view of a
// project is derived from the job's real state on a real node, so the
// batch cannot report Succeeded for work no machine ever finished.

/// core → api → web, with api and web sharing a monorepo working tree.
fn monorepo_graph() -> ProjectGraph {
    ProjectGraph::new([
        ProjectDep::new(CORE).in_worktree("/srv/acme"),
        ProjectDep::new(API).after([CORE]).in_worktree("/srv/acme"),
        ProjectDep::new(WEB).after([CORE]).in_worktree("/srv/acme"),
    ])
    .expect("an acyclic graph")
}

#[test]
fn a_batch_project_is_running_only_once_a_real_node_holds_its_job() {
    let now = at(9, 0);
    let fleet = fleet_of(now, &[("node-a", &[CORE, API, WEB], 2)]);
    let mut queue = WorkQueue::new();
    let mut batch = BatchRun::plan("batch-1", "advance the roadmap", monorepo_graph())
        .expect("planned")
        .requested_by(ALICE);

    assert_eq!(batch.order(), [CORE, API, WEB]);

    // Releasing submits; it does not run.
    assert_eq!(batch.advance(&mut queue).expect("advance"), vec![CORE]);
    assert_eq!(
        batch.state_of(&queue, CORE),
        BatchState::Queued,
        "released and submitted is Queued — the batch does not call it \
         Running on its own say-so"
    );

    // Only the fleet can make it Running, and the batch reads that back
    // out of the queue rather than recording it.
    queue.dispatch(&fleet, now);
    assert_eq!(
        batch.state_of(&queue, CORE),
        BatchState::Running,
        "Running is derived from the job being Claimed by a node"
    );
    assert_eq!(batch.state_of(&queue, API), BatchState::Waiting);
}

#[test]
fn the_monorepo_exclusion_holds_against_jobs_that_are_genuinely_in_flight() {
    let now = at(9, 0);
    let fleet = fleet_of(now, &[("node-a", &[CORE, API, WEB], 4)]);
    let mut queue = WorkQueue::new();
    let mut batch = BatchRun::plan("batch-1", "advance the roadmap", monorepo_graph())
        .expect("planned")
        .requested_by(ALICE);

    // core runs and finishes on a real node.
    batch.advance(&mut queue).expect("release core");
    let placed = queue.dispatch(&fleet, now);
    assert_eq!(placed.len(), 1);
    let attempt = queue
        .job(&batch.job_id(CORE))
        .map(|j| j.attempt)
        .expect("claimed");
    queue
        .complete(&batch.job_id(CORE), "node-a", attempt, true)
        .expect("core succeeds");

    // api and web are both unblocked and the node has four free slots —
    // but they declared the same working tree, so exactly one is
    // released. git is a single-writer store, and a scheduler that
    // already holds the graph does not have to discover that by racing
    // on .git/index.lock.
    let released = batch.advance(&mut queue).expect("advance");
    assert_eq!(
        released,
        vec![API.to_string()],
        "one of the two monorepo archetypes goes, not both"
    );
    assert_eq!(batch.state_of(&queue, WEB), BatchState::Waiting);

    // The other waits for the tree, not for capacity: the node is idle
    // enough to take it and still does not.
    queue.dispatch(&fleet, now);
    assert!(
        queue.free_slots(&fleet, "node-a", now) > 0,
        "the node has room, so what is holding web back is the worktree"
    );
    assert_eq!(
        batch.advance(&mut queue).expect("advance"),
        Vec::<String>::new()
    );

    // Once api settles the tree frees and web takes its turn.
    let attempt = queue
        .job(&batch.job_id(API))
        .map(|j| j.attempt)
        .expect("claimed");
    queue
        .complete(&batch.job_id(API), "node-a", attempt, true)
        .expect("api succeeds");
    assert_eq!(batch.advance(&mut queue).expect("advance"), vec![WEB]);
}

#[test]
fn a_project_that_fails_on_a_node_stops_its_dependents_and_nothing_else() {
    let now = at(9, 0);
    let fleet = fleet_of(now, &[("node-a", &[CORE, API, WEB], 4)]);
    let mut queue = WorkQueue::new();

    // core → api, with web independent of both, so "stops its
    // dependents rather than the whole batch" has something to prove.
    let graph = ProjectGraph::new([
        ProjectDep::new(CORE),
        ProjectDep::new(API).after([CORE]),
        ProjectDep::new(WEB),
    ])
    .expect("acyclic");
    let mut batch = BatchRun::plan("batch-1", "advance the roadmap", graph).expect("planned");

    batch.advance(&mut queue).expect("release the roots");
    queue.dispatch(&fleet, now);

    // core fails on the node.
    let attempt = queue
        .job(&batch.job_id(CORE))
        .map(|j| j.attempt)
        .expect("claimed");
    queue
        .complete(&batch.job_id(CORE), "node-a", attempt, false)
        .expect("core fails");

    batch.advance(&mut queue).expect("advance");
    assert_eq!(batch.state_of(&queue, CORE), BatchState::Failed);
    assert_eq!(
        batch.state_of(&queue, API),
        BatchState::Skipped,
        "the dependent is cancelled by the upstream failure"
    );
    assert_ne!(
        batch.state_of(&queue, WEB),
        BatchState::Skipped,
        "and the independent project is untouched — a failure stops its \
         dependents, not the batch"
    );

    // Finish web and the whole run is settled with one rollup, not three
    // unrelated streams.
    let attempt = queue
        .job(&batch.job_id(WEB))
        .map(|j| j.attempt)
        .expect("claimed");
    queue
        .complete(&batch.job_id(WEB), "node-a", attempt, true)
        .expect("web succeeds");
    assert!(batch.is_settled(&queue));

    let status = batch.status(&queue);
    assert_eq!(status.rows.len(), 3);
    let rows: Vec<&str> = status.rows.iter().map(|r| r.project.as_str()).collect();
    let position = |p: &str| rows.iter().position(|r| *r == p).expect("row present");
    assert!(
        position(CORE) < position(API),
        "the rollup is one answer in dependency order, not N unrelated \
         streams: {rows:?}"
    );
}

#[test]
fn a_batch_whose_projects_live_on_different_machines_lands_each_on_one_that_serves_it() {
    let now = at(9, 0);
    // No node serves everything: node-a runs core and api, node-b web.
    let fleet = fleet_of(now, &[("node-a", &[CORE, API], 2), ("node-b", &[WEB], 2)]);
    let mut queue = WorkQueue::new();
    let graph = ProjectGraph::new([
        ProjectDep::new(CORE),
        ProjectDep::new(API).after([CORE]),
        ProjectDep::new(WEB),
    ])
    .expect("acyclic");
    let mut batch = BatchRun::plan("batch-1", "advance the roadmap", graph).expect("planned");

    batch.advance(&mut queue).expect("release the roots");
    let placed = queue.dispatch(&fleet, now);

    let core_job = batch.job_id(CORE);
    let web_job = batch.job_id(WEB);
    assert!(placed.contains(&(core_job, "node-a".into())));
    assert!(
        placed.contains(&(web_job, "node-b".into())),
        "each project lands on a node that serves it, which the batch \
         never had to know about: {placed:?}"
    );
}

#[test]
fn a_cyclic_batch_is_refused_by_naming_the_cycle_before_anything_is_submitted() {
    let cyclic = ProjectGraph::new([
        ProjectDep::new(CORE).after([WEB]),
        ProjectDep::new(API).after([CORE]),
        ProjectDep::new(WEB).after([API]),
    ])
    .expect("construction does not order");

    let refused =
        BatchRun::plan("batch-1", "advance", cyclic).expect_err("no order satisfies a cycle");
    let message = refused.to_string();
    for project in [CORE, API, WEB] {
        assert!(
            message.contains(project),
            "the refusal names the set no order can satisfy — {message}"
        );
    }
}

// Team configuration supplies the governance policy and budget rules used
// to seal a fleet bundle. This fixture tests their compatibility across the
// CLI and engine crates. Production publication still requires the caller
// to construct and submit the bundle.

fn org_roster(members: Vec<TeamMember>) -> TeamConfig {
    TeamConfig {
        version: 1,
        default_role: TeamRole::Viewer,
        identity_provider: None,
        budgets: org_budgets(),
        policy: org_policy(),
        directory: DirectoryConfig::with_groups(vec![
            GroupRole {
                group: "eng-*".into(),
                role: TeamRole::Developer,
            },
            GroupRole {
                group: "platform-admins".into(),
                role: TeamRole::Admin,
            },
        ]),
        members,
    }
}

fn member(id: &str, role: TeamRole) -> TeamMember {
    TeamMember {
        id: id.to_string(),
        name: None,
        role,
        active: true,
    }
}

/// The user resource an IdP posts, in a named group.
fn scim_user(id: &str, group: &str) -> ScimUser {
    let mut user = ScimUser::new(id);
    user.groups = vec![ScimValue {
        value: Some(group.to_string()),
        display: Some(group.to_string()),
        kind: None,
        primary: None,
    }];
    user
}

#[test]
fn the_roster_a_team_hand_writes_is_the_bundle_a_fleet_publishes() {
    let now = at(9, 0);
    let roster = org_roster(vec![member(ALICE, TeamRole::Owner)]);

    // The seal takes the roster's own two governance fields. No
    // translation layer, no second vocabulary to drift.
    let bundle = PolicyBundle::seal(1, roster.policy.clone(), roster.budgets.clone());
    assert!(bundle.verify());

    let mut fleet = FleetRegistry::with_lease_ttl_secs(LEASE_TTL);
    fleet.register(FleetNode::new("node-a", "a.acme.example", "0.1.0", now));
    fleet.publish(bundle.clone()).expect("publish generation 1");

    // A node that has not applied it is named as drifted, and the
    // fleet is not converged until it has.
    let status = fleet.status(now);
    assert_eq!(status.ready, 1);
    assert_eq!(status.drifted, vec!["node-a".to_string()]);
    assert!(!status.is_converged());

    // The heartbeat hands back the bundle the node still has to apply.
    let owed = fleet
        .heartbeat("node-a", now, 0, "")
        .expect("registered")
        .expect("a node behind the fleet is given the bundle");
    assert_eq!(owed.generation, 1);

    assert!(
        fleet
            .heartbeat("node-a", now, bundle.generation, &bundle.digest)
            .expect("registered")
            .is_none(),
        "a node already running the published generation is owed \
         nothing, so the bundle is on the wire only when it is needed"
    );
    assert!(fleet.status(now).is_converged());

    // And what the node now enforces is the org's rule, arriving from
    // the roster rather than from anything local.
    let decision = bundle.engine().evaluate(&PolicyRequest::tool("Bash"));
    assert_eq!(decision.effect, PolicyEffect::Deny);
}

#[test]
fn a_node_applying_the_org_bundle_can_only_ever_be_tightened_by_it() {
    let roster = org_roster(vec![member(ALICE, TeamRole::Owner)]);
    let bundle = PolicyBundle::seal(1, roster.policy.clone(), roster.budgets.clone());

    // A node whose own roster tries to bless the tool the org denies.
    let local = GovernancePolicy {
        tool_allowlist: vec!["Bash".into()],
        ..GovernancePolicy::default()
    };
    let merged = bundle.merge_local(&local);
    assert_eq!(
        merged.evaluate(&PolicyRequest::tool("Bash")).effect,
        PolicyEffect::Deny,
        "a local allow cannot loosen an org denial: evaluate folds max \
         over every matching rule, so a union is the tighter of the two"
    );

    // The other direction still works — local policy is additive.
    let stricter = GovernancePolicy {
        tool_denylist: vec!["WebFetch".into()],
        ..GovernancePolicy::default()
    };
    assert_eq!(
        bundle
            .merge_local(&stricter)
            .evaluate(&PolicyRequest::tool("WebFetch"))
            .effect,
        PolicyEffect::Deny,
        "and a node may still deny more than the org does"
    );

    // A rollback to a looser generation is refused by the registry, so
    // the fleet cannot be walked backwards onto it.
    let mut fleet = FleetRegistry::new();
    fleet
        .publish(PolicyBundle::seal(5, roster.policy.clone(), vec![]))
        .expect("publish 5");
    fleet
        .publish(PolicyBundle::seal(4, GovernancePolicy::default(), vec![]))
        .expect_err("an older generation is refused");
    assert_eq!(fleet.bundle().generation, 5);
}

#[test]
fn scim_writes_the_roster_the_fleets_governance_is_sealed_from() {
    let mut roster = org_roster(vec![member(ALICE, TeamRole::Owner)]);
    let before = PolicyBundle::seal(1, roster.policy.clone(), roster.budgets.clone());

    // The IdP provisions Bob into an engineering group.
    let outcome = apply_scim(
        &mut roster,
        &ScimRequest::Create {
            user: scim_user(BOB, "eng-platform"),
        },
    )
    .expect("provisioned");
    assert_eq!(outcome.role, TeamRole::Developer, "mapped by group");
    assert_eq!(roster.role_of(BOB), Some(TeamRole::Developer));

    // Provisioning a person changes the membership and *not* the
    // governance — the bundle's digest is over policy and budgets, so
    // a roster edit that touches neither cannot churn the fleet.
    let after = PolicyBundle::seal(1, roster.policy.clone(), roster.budgets.clone());
    assert_eq!(
        before.digest, after.digest,
        "a membership change is not a policy change, so nodes are not \
         asked to re-apply anything"
    );
}

#[test]
fn a_deprovisioned_member_is_refused_by_name_rather_than_falling_back_to_a_default() {
    let mut roster = org_roster(vec![
        member(ALICE, TeamRole::Owner),
        member(BOB, TeamRole::Developer),
    ]);

    // The deactivation both Okta and Entra actually send: a PATCH
    // setting `active` false, not a DELETE.
    let request: ScimRequest = serde_json::from_value(serde_json::json!({
        "op": "patch",
        "id": BOB,
        "Operations": [{ "op": "replace", "path": "active", "value": false }],
    }))
    .expect("a provider's own wire shape");
    apply_scim(&mut roster, &request).expect("deactivated");

    // The row is kept, so reactivation is one field rather than a
    // re-provision that loses their history.
    assert!(roster.row(BOB).is_some());
    assert_eq!(roster.member_for(&identity(BOB)), None);

    let refused = resolve_role_with_directory(
        Some(&roster),
        &identity(BOB),
        false,
        &DirectoryOutcome::NoMatch,
    )
    .expect_err("a deprovisioned member gets no role");
    assert!(
        refused.to_string().contains(BOB),
        "and the refusal names them rather than silently handing back \
         the roster's default_role — {refused}"
    );

    // Alice is untouched by any of it.
    assert_eq!(
        resolve_role_with_directory(
            Some(&roster),
            &identity(ALICE),
            false,
            &DirectoryOutcome::NoMatch
        )
        .expect("the roster beats the directory"),
        TeamRole::Owner
    );
}

#[test]
fn a_directory_outage_refuses_automation_too() {
    let roster = org_roster(vec![member(ALICE, TeamRole::Owner)]);
    let unknown = identity("carol@acme.example");

    // Not on the roster, and the directory did not answer.
    let outage = DirectoryOutcome::Unavailable("connection refused".into());
    resolve_role_with_directory(Some(&roster), &unknown, false, &outage)
        .expect_err("an unanswered question is a refusal");
    resolve_role_with_directory(Some(&roster), &unknown, true, &outage).expect_err(
        "and it is checked before the CI shortcut — fail closed that \
         exempts automation is not fail closed, and CI is the identity \
         most likely to be running during an outage",
    );

    // An *answered* no is data, and falls through to the roster's own
    // default rather than refusing.
    assert_eq!(
        resolve_role_with_directory(Some(&roster), &unknown, false, &DirectoryOutcome::NoMatch)
            .expect("an answered no is data"),
        TeamRole::Viewer
    );
}

// ── The whole chain, once, in order ─────────────────────────────────

/// One org, one policy, two machines, a three-project batch, a machine
/// taken away for maintenance mid-run, and a session that carries on
/// somewhere else.
///
/// Every section above isolates one join; this walks all five in the
/// order an operator would meet them, so a regression that only shows
/// up in composition has somewhere to fail.
#[test]
fn one_org_runs_a_batch_across_a_fleet_and_survives_losing_a_machine() {
    let day = at(9, 0);

    // 1. the directory provisions the person who will request
    //    the work, into the roster the org's governance lives in.
    let mut roster = org_roster(vec![member(ALICE, TeamRole::Owner)]);
    apply_scim(
        &mut roster,
        &ScimRequest::Create {
            user: scim_user(BOB, "eng-platform"),
        },
    )
    .expect("provisioned");
    assert_eq!(
        resolve_role_with_directory(
            Some(&roster),
            &identity(BOB),
            false,
            &DirectoryOutcome::NoMatch
        )
        .expect("rostered"),
        TeamRole::Developer,
    );

    // 2. that same roster's policy and budgets are sealed and
    //    published; both machines apply it and the fleet converges.
    let bundle = PolicyBundle::seal(1, roster.policy.clone(), roster.budgets.clone());
    // node-a runs core and api; node-b is the only machine that serves
    // web, which is what makes taking it away later mean something.
    let mut fleet = fleet_of(day, &[("node-a", &[CORE, API], 2), ("node-b", &[WEB], 1)]);
    fleet.publish(bundle.clone()).expect("publish");
    for node in ["node-a", "node-b"] {
        fleet
            .heartbeat(node, day, bundle.generation, &bundle.digest)
            .expect("registered");
    }
    let status = fleet.status(day);
    assert_eq!((status.ready, status.total), (2, 2));
    assert!(status.is_converged(), "both machines run generation 1");

    // 3. a dependency-ordered batch is planned and its first
    //    project released into the queue.
    let mut queue = WorkQueue::new();
    let graph = ProjectGraph::new([
        ProjectDep::new(CORE),
        ProjectDep::new(API).after([CORE]),
        ProjectDep::new(WEB).after([CORE]),
    ])
    .expect("acyclic");
    let mut batch = BatchRun::plan("nightly", "advance the roadmap", graph)
        .expect("planned")
        .requested_by(BOB);
    assert_eq!(batch.advance(&mut queue).expect("advance"), vec![CORE]);

    // 4. the queue places it on a machine that serves it, and
    //    the work is attributed to the person the directory provisioned.
    assert_eq!(
        queue.dispatch(&fleet, day),
        vec![(batch.job_id(CORE), "node-a".to_string())]
    );
    assert_eq!(
        queue
            .job(&batch.job_id(CORE))
            .map(|j| j.unit.requested_by.as_str()),
        Some(BOB),
        "a batch's jobs are attributed the way a single submission is"
    );

    // core finishes; api and web are released and dispatched, and web
    // goes to the only machine that serves it.
    let core_attempt = queue
        .job(&batch.job_id(CORE))
        .map(|j| j.attempt)
        .expect("claimed");
    queue
        .complete(&batch.job_id(CORE), "node-a", core_attempt, true)
        .expect("core succeeds");
    let released = batch.advance(&mut queue).expect("advance");
    assert_eq!(released.len(), 2, "no shared worktree here, so both go");
    let placed = queue.dispatch(&fleet, day);
    assert!(placed.contains(&(batch.job_id(WEB), "node-b".into())));
    assert!(placed.contains(&(batch.job_id(API), "node-a".into())));

    // 5. node-b gets some way into web's session and
    //    checkpoints it.
    let web_job = batch.job_id(WEB);
    let web_attempt = queue.job(&web_job).map(|j| j.attempt).expect("claimed");
    let session = working_session("sess-web");
    queue
        .report(
            &web_job,
            "node-b",
            web_attempt,
            0,
            phase_event(WorkflowPhase::Executing),
        )
        .expect("the holder reports");
    queue
        .checkpoint(
            &web_job,
            "node-b",
            web_attempt,
            snapshot_of(&session, 4, day),
        )
        .expect("the holder checkpoints");
    let read_so_far = queue
        .events(&web_job, 0)
        .map(<[_]>::len)
        .unwrap_or_default();
    assert_eq!(read_so_far, 1);

    // The operator takes node-b away for maintenance: cordon, then
    // drain. One wire method does both in that order so a client cannot
    // get it wrong; here they are the two calls it is made of.
    fleet.cordon("node-b", true).expect("cordon");
    assert_eq!(
        queue.free_slots(&fleet, "node-b", day),
        0,
        "the cordon is what stops the drained work coming straight back"
    );
    assert_eq!(
        queue.drain(&fleet, "node-b").expect("drain"),
        vec![web_job.clone()]
    );

    // Nowhere else serves web, so it waits rather than landing on the
    // machine that is going away — this is the cordon being real.
    assert_eq!(queue.place(&fleet, day), None);
    assert_eq!(batch.state_of(&queue, WEB), BatchState::Queued);
    assert!(!batch.is_settled(&queue));

    // A replacement machine joins the fleet, takes generation 1 like
    // everybody else, and picks the job up where node-b left it.
    let mut node_c = FleetNode::new("node-c", "c.acme.example", "0.1.0", day);
    node_c.projects = vec![WEB.to_string()];
    fleet.register(node_c);
    fleet
        .heartbeat("node-c", day, bundle.generation, &bundle.digest)
        .expect("registered");
    assert!(fleet.status(day).is_converged(), "the newcomer is current");

    let (job, node) = queue.place(&fleet, day).expect("placeable on the newcomer");
    assert_eq!((job.as_str(), node.as_str()), (web_job.as_str(), "node-c"));
    let resumed_attempt = queue.claim(&fleet, &web_job, "node-c", day).expect("claim");
    assert_eq!(resumed_attempt, web_attempt + 1);

    // The session it resumes from is the one node-b checkpointed, at
    // the turn boundary rather than mid-turn.
    let restored = queue
        .job(&web_job)
        .and_then(|j| j.snapshot.as_ref())
        .expect("the checkpoint crossed the machine")
        .restore()
        .expect("restorable");
    assert_eq!(restored.session.session_id(), "sess-web");
    assert_eq!(restored.iteration.iteration, 4);
    assert_eq!(restored.session.status(), SessionStatus::Awaiting);

    // The reader's stream appends rather than rewinding: what node-c
    // reports lands past what node-b already delivered.
    queue
        .report(
            &web_job,
            "node-c",
            resumed_attempt,
            0,
            phase_event(WorkflowPhase::Verifying),
        )
        .expect("the new holder reports from its own sequence zero");
    assert_eq!(
        queue.events(&web_job, read_so_far).map(<[_]>::len),
        Some(1),
        "one new event past the reader's cursor"
    );

    // 6. The batch settles, and the governance that was published at
    //    the top is still what the surviving fleet enforces.
    for project in [API, WEB] {
        let id = batch.job_id(project);
        let (node, attempt) = queue
            .job(&id)
            .and_then(|j| Some((j.node_id.clone()?, j.attempt)))
            .expect("claimed");
        queue.complete(&id, &node, attempt, true).expect("succeeds");
    }
    batch.advance(&mut queue).expect("advance");
    assert!(batch.is_settled(&queue));
    assert_eq!(batch.status(&queue).rows.len(), 3);
    for project in [CORE, API, WEB] {
        assert_eq!(batch.state_of(&queue, project), BatchState::Succeeded);
    }

    assert_eq!(fleet.bundle().generation, 1);
    assert_eq!(
        fleet
            .bundle()
            .engine()
            .evaluate(&PolicyRequest::tool("Bash"))
            .effect,
        PolicyEffect::Deny,
        "the org's rule outlived the machine that went away"
    );
}

#[test]
fn losing_the_only_machine_that_serves_a_project_stalls_it_without_failing_it() {
    let day = at(9, 0);
    let mut fleet = fleet_of(day, &[("node-a", &[CORE], 1), ("node-b", &[WEB], 1)]);
    let mut queue = WorkQueue::new();
    let graph = ProjectGraph::new([ProjectDep::new(CORE), ProjectDep::new(WEB)]).expect("acyclic");
    let mut batch = BatchRun::plan("nightly", "advance", graph).expect("planned");

    batch.advance(&mut queue).expect("release both");
    queue.dispatch(&fleet, day);

    // node-b vanishes for good — no cordon, no drain, just silence.
    let later = day + Duration::seconds(LEASE_TTL * 4);
    fleet
        .heartbeat("node-a", later, 0, "")
        .expect("node-a alive");
    assert_eq!(queue.reclaim(&fleet, later), vec![batch.job_id(WEB)]);

    // web is Queued and stays Queued: nothing serves it, and the batch
    // says so rather than reporting a failure that never happened.
    assert_eq!(batch.state_of(&queue, WEB), BatchState::Queued);
    assert_eq!(queue.place(&fleet, later), None);
    assert!(!batch.is_settled(&queue));
    assert_eq!(
        queue.count_in(JobState::Failed),
        0,
        "a machine going away is not a job failing — nothing downstream \
         of web is cancelled on the strength of it"
    );
}
