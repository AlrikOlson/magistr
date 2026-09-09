use super::*;

use chrono::TimeZone;

use super::super::fleet_node::FleetNode;

const LEASE_TTL: i64 = 60;

fn at(secs: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(1_800_000_000 + secs, 0)
        .single()
        .expect("a real instant")
}

/// A fleet whose nodes were all last seen at `at(0)`.
fn fleet(nodes: Vec<FleetNode>) -> FleetRegistry {
    let mut fleet = FleetRegistry::with_lease_ttl_secs(LEASE_TTL);
    for node in nodes {
        fleet.register(node);
    }
    fleet
}

fn node(id: &str) -> FleetNode {
    FleetNode::new(id, format!("{id}.local"), "1.0.0", at(0))
}

fn unit(id: &str, project: &str) -> WorkUnit {
    WorkUnit::new(id, project, format!("run {id}"))
}

fn msg(text: &str) -> SessionEvent {
    SessionEvent::MessageReceived {
        content: text.to_owned(),
    }
}

// ── The ledger ──────────────────────────────────────────────────────

#[test]
fn a_submitted_job_is_queued_and_a_resubmission_is_refused() {
    let mut queue = WorkQueue::new();
    let job = queue.submit(unit("j1", "magistr")).expect("submitted");
    assert_eq!(job.state, JobState::Queued);
    assert_eq!(queue.len(), 1);

    let err = queue
        .submit(unit("j1", "magistr"))
        .expect_err("a duplicate id is refused");
    assert!(err.to_string().contains("already been submitted"), "{err}");
    assert_eq!(queue.len(), 1, "the first job is untouched");
}

#[test]
fn jobs_come_back_in_submission_order_not_id_order() {
    let mut queue = WorkQueue::new();
    for id in ["z", "a", "m"] {
        queue.submit(unit(id, "magistr")).expect("submitted");
    }
    let ids: Vec<&str> = queue.jobs().into_iter().map(Job::id).collect();
    assert_eq!(ids, ["z", "a", "m"]);
}

#[test]
fn forgetting_a_job_drops_it_from_the_order_too() {
    let mut queue = WorkQueue::new();
    queue.submit(unit("j1", "magistr")).expect("submitted");
    queue.submit(unit("j2", "magistr")).expect("submitted");

    assert!(queue.forget("j1"));
    assert!(!queue.forget("j1"));
    let ids: Vec<&str> = queue.jobs().into_iter().map(Job::id).collect();
    assert_eq!(ids, ["j2"]);
    assert!(!queue.is_empty());
}

// ── Capacity ────────────────────────────────────────────────────────

#[test]
fn in_flight_is_counted_from_the_jobs_and_falls_back_on_release() {
    let fleet = fleet(vec![node("n1").with_capacity(4)]);
    let mut queue = WorkQueue::new();
    queue.submit(unit("j1", "magistr")).expect("submitted");
    queue.submit(unit("j2", "magistr")).expect("submitted");

    assert_eq!(queue.in_flight("n1"), 0);
    let a1 = queue.claim(&fleet, "j1", "n1", at(1)).expect("claimed");
    queue.claim(&fleet, "j2", "n1", at(1)).expect("claimed");
    assert_eq!(queue.in_flight("n1"), 2);
    assert_eq!(queue.free_slots(&fleet, "n1", at(1)), 2);

    queue.complete("j1", "n1", a1, true).expect("completed");
    assert_eq!(queue.in_flight("n1"), 1, "a settled job frees its slot");
}

#[test]
fn a_node_with_no_declared_capacity_holds_exactly_one_job() {
    let fleet = fleet(vec![node("n1")]);
    let mut queue = WorkQueue::new();
    queue.submit(unit("j1", "magistr")).expect("submitted");
    queue.submit(unit("j2", "magistr")).expect("submitted");

    queue.claim(&fleet, "j1", "n1", at(1)).expect("claimed");
    let err = queue
        .claim(&fleet, "j2", "n1", at(1))
        .expect_err("the node is full");
    assert!(err.to_string().contains("capacity of 1"), "{err}");
}

#[test]
fn an_unknown_or_unready_node_has_no_free_slots() {
    let fleet = fleet(vec![node("n1")]);
    let queue = WorkQueue::new();
    assert_eq!(queue.free_slots(&fleet, "ghost", at(1)), 0);
    assert_eq!(queue.free_slots(&fleet, "n1", at(1)), 1);
    assert_eq!(
        queue.free_slots(&fleet, "n1", at(LEASE_TTL + 1)),
        0,
        "a stale node is not offered work"
    );
}

// ── Scheduling ──────────────────────────────────────────────────────

#[test]
fn placement_picks_a_ready_node_that_serves_the_project() {
    let fleet = fleet(vec![
        node("n-other").with_projects(vec!["other".into()]),
        node("n-right").with_projects(vec!["magistr".into()]),
    ]);
    let mut queue = WorkQueue::new();
    queue.submit(unit("j1", "magistr")).expect("submitted");

    assert_eq!(
        queue.place(&fleet, at(1)),
        Some(("j1".to_owned(), "n-right".to_owned()))
    );
}

#[test]
fn a_node_declaring_no_projects_is_general_purpose() {
    let fleet = fleet(vec![node("n1")]);
    let mut queue = WorkQueue::new();
    queue
        .submit(unit("j1", "anything-at-all"))
        .expect("submitted");
    assert_eq!(
        queue.place(&fleet, at(1)),
        Some(("j1".to_owned(), "n1".to_owned()))
    );
}

#[test]
fn nothing_is_placed_on_a_stale_or_unreachable_node() {
    let fleet = fleet(vec![node("n1")]);
    let mut queue = WorkQueue::new();
    queue.submit(unit("j1", "magistr")).expect("submitted");

    assert!(queue.place(&fleet, at(1)).is_some());
    assert!(
        queue.place(&fleet, at(LEASE_TTL + 1)).is_none(),
        "Stale is stricter than the acceptance asks, on purpose"
    );
    let miss = LEASE_TTL * NodeHealth::MISS_ALLOWANCE;
    assert!(queue.place(&fleet, at(miss + 1)).is_none());
}

#[test]
fn placement_prefers_the_emptiest_node_and_breaks_ties_on_the_lower_id() {
    let fleet = fleet(vec![
        node("n-a").with_capacity(2),
        node("n-b").with_capacity(2),
    ]);
    let mut queue = WorkQueue::new();
    for id in ["j1", "j2", "j3"] {
        queue.submit(unit(id, "magistr")).expect("submitted");
    }

    // A tie on free slots goes to the lower id.
    assert_eq!(queue.place(&fleet, at(1)).expect("placed").1, "n-a");
    queue.claim(&fleet, "j1", "n-a", at(1)).expect("claimed");
    // n-b now has more room, so it wins on its own.
    assert_eq!(queue.place(&fleet, at(1)).expect("placed").1, "n-b");
}

#[test]
fn placement_skips_a_job_no_node_serves_and_takes_the_next_one() {
    let fleet = fleet(vec![node("n1").with_projects(vec!["magistr".into()])]);
    let mut queue = WorkQueue::new();
    queue
        .submit(unit("j-orphan", "nowhere"))
        .expect("submitted");
    queue.submit(unit("j-ok", "magistr")).expect("submitted");

    assert_eq!(
        queue.place(&fleet, at(1)),
        Some(("j-ok".to_owned(), "n1".to_owned())),
        "head-of-line blocking would strand the whole queue"
    );
}

#[test]
fn placement_is_pure_and_claiming_is_what_moves_the_job() {
    let fleet = fleet(vec![node("n1")]);
    let mut queue = WorkQueue::new();
    queue.submit(unit("j1", "magistr")).expect("submitted");

    let first = queue.place(&fleet, at(1));
    assert_eq!(
        queue.place(&fleet, at(1)),
        first,
        "placement mutates nothing"
    );

    queue.claim(&fleet, "j1", "n1", at(1)).expect("claimed");
    assert!(queue.place(&fleet, at(1)).is_none());
}

#[test]
fn a_claim_is_refused_for_every_ineligible_reason_by_name() {
    let fleet = fleet(vec![node("n1").with_projects(vec!["magistr".into()])]);
    let mut queue = WorkQueue::new();
    queue.submit(unit("j1", "magistr")).expect("submitted");
    queue
        .submit(unit("j-elsewhere", "other"))
        .expect("submitted");

    let unknown_job = queue
        .claim(&fleet, "nope", "n1", at(1))
        .expect_err("unknown job");
    assert!(
        unknown_job.to_string().contains("no job 'nope'"),
        "{unknown_job}"
    );

    let unknown_node = queue
        .claim(&fleet, "j1", "ghost", at(1))
        .expect_err("unknown node");
    assert!(
        unknown_node.to_string().contains("not registered"),
        "{unknown_node}"
    );

    let wrong_project = queue
        .claim(&fleet, "j-elsewhere", "n1", at(1))
        .expect_err("wrong project");
    assert!(
        wrong_project.to_string().contains("does not serve"),
        "{wrong_project}"
    );

    let stale = queue
        .claim(&fleet, "j1", "n1", at(LEASE_TTL + 1))
        .expect_err("stale node");
    assert!(stale.to_string().contains("is stale"), "{stale}");

    queue.claim(&fleet, "j1", "n1", at(1)).expect("claimed");
    let already = queue
        .claim(&fleet, "j1", "n1", at(1))
        .expect_err("already claimed");
    assert!(already.to_string().contains("is claimed"), "{already}");
}

#[test]
fn a_node_pulling_work_gets_only_what_it_serves() {
    let fleet = fleet(vec![node("n1").with_projects(vec!["magistr".into()])]);
    let mut queue = WorkQueue::new();
    queue
        .submit(unit("j-elsewhere", "other"))
        .expect("submitted");
    queue.submit(unit("j1", "magistr")).expect("submitted");

    let claimed = queue.claim_next(&fleet, "n1", at(1)).expect("no error");
    assert_eq!(claimed, Some(("j1".to_owned(), 1)));

    assert_eq!(
        queue.claim_next(&fleet, "n1", at(1)).expect("no error"),
        None,
        "the node is full and the other job is not for it"
    );
}

#[test]
fn dispatch_fills_the_fleet_and_stops_when_it_is_full() {
    let fleet = fleet(vec![
        node("n-a").with_capacity(2),
        node("n-b").with_capacity(1),
    ]);
    let mut queue = WorkQueue::new();
    for id in ["j1", "j2", "j3", "j4"] {
        queue.submit(unit(id, "magistr")).expect("submitted");
    }

    let placed = queue.dispatch(&fleet, at(1));
    assert_eq!(placed.len(), 3, "capacity 2 + 1");
    assert_eq!(queue.count_in(JobState::Claimed), 3);
    assert_eq!(queue.count_in(JobState::Queued), 1);
    assert_eq!(queue.in_flight("n-a"), 2);
    assert_eq!(queue.in_flight("n-b"), 1);
}

#[test]
fn dispatch_onto_an_empty_fleet_places_nothing_and_terminates() {
    let fleet = fleet(vec![]);
    let mut queue = WorkQueue::new();
    queue.submit(unit("j1", "magistr")).expect("submitted");
    assert!(queue.dispatch(&fleet, at(1)).is_empty());
    assert_eq!(queue.count_in(JobState::Queued), 1);
}

// ── Reporting ───────────────────────────────────────────────────────

#[test]
fn a_holders_events_reach_the_client_in_order_through_the_cursor() {
    let fleet = fleet(vec![node("n1")]);
    let mut queue = WorkQueue::new();
    queue.submit(unit("j1", "magistr")).expect("submitted");
    let attempt = queue.claim(&fleet, "j1", "n1", at(1)).expect("claimed");

    // Out of order on the wire...
    queue
        .report("j1", "n1", attempt, 1, msg("b"))
        .expect("taken");
    assert_eq!(queue.events("j1", 0).expect("a known job").len(), 0);
    queue
        .report("j1", "n1", attempt, 0, msg("a"))
        .expect("taken");

    // ...in order to the reader.
    let events = queue.events("j1", 0).expect("a known job");
    assert_eq!(events, [msg("a"), msg("b")]);
    assert_eq!(queue.events("j1", 2).expect("a known job"), []);
    assert!(queue.events("nope", 0).is_none());
}

#[test]
fn a_node_that_does_not_hold_the_job_cannot_report_or_complete_it() {
    let fleet = fleet(vec![node("n1"), node("n2")]);
    let mut queue = WorkQueue::new();
    queue.submit(unit("j1", "magistr")).expect("submitted");
    let attempt = queue.claim(&fleet, "j1", "n1", at(1)).expect("claimed");

    let reported = queue
        .report("j1", "n2", attempt, 0, msg("x"))
        .expect_err("not the holder");
    assert!(reported.to_string().contains("not held by"), "{reported}");

    let completed = queue
        .complete("j1", "n2", attempt, true)
        .expect_err("not the holder");
    assert!(completed.to_string().contains("not held by"), "{completed}");
}

#[test]
fn completing_records_the_outcome_and_a_queued_job_cannot_be_completed() {
    let fleet = fleet(vec![node("n1").with_capacity(2)]);
    let mut queue = WorkQueue::new();
    queue.submit(unit("j1", "magistr")).expect("submitted");
    queue.submit(unit("j2", "magistr")).expect("submitted");

    let a1 = queue.claim(&fleet, "j1", "n1", at(1)).expect("claimed");
    assert_eq!(
        queue.complete("j1", "n1", a1, false).expect("settled"),
        JobState::Failed
    );

    let a2 = queue.claim(&fleet, "j2", "n1", at(1)).expect("claimed");
    assert_eq!(
        queue.complete("j2", "n1", a2, true).expect("settled"),
        JobState::Completed
    );

    let twice = queue
        .complete("j2", "n1", a2, true)
        .expect_err("already settled");
    assert!(twice.to_string().contains("not held by"), "{twice}");
}

// ── Reclaim ─────────────────────────────────────────────────────────

#[test]
fn a_node_that_goes_unreachable_mid_job_has_its_work_reclaimed() {
    let mut fleet = fleet(vec![node("n1")]);
    let mut queue = WorkQueue::new();
    queue.submit(unit("j1", "magistr")).expect("submitted");
    queue.claim(&fleet, "j1", "n1", at(1)).expect("claimed");

    let gone = LEASE_TTL * NodeHealth::MISS_ALLOWANCE + 1;
    assert!(
        queue.reclaim(&fleet, at(1)).is_empty(),
        "a live holder keeps it"
    );
    assert_eq!(queue.reclaim(&fleet, at(gone)), ["j1"]);
    assert_eq!(queue.count_in(JobState::Queued), 1);
    assert!(queue.job("j1").expect("a known job").node_id.is_none());

    // The work is not lost: a live node takes it, as a second attempt.
    fleet.register(FleetNode::new("n2", "n2.local", "1.0.0", at(gone)));
    assert_eq!(
        queue.place(&fleet, at(gone)),
        Some(("j1".to_owned(), "n2".to_owned()))
    );
    let attempt = queue
        .claim(&fleet, "j1", "n2", at(gone))
        .expect("re-claimed");
    assert_eq!(attempt, 2, "the second hand-out is the second attempt");
}

#[test]
fn a_job_whose_holder_was_forgotten_entirely_is_reclaimed_too() {
    let mut fleet = fleet(vec![node("n1")]);
    let mut queue = WorkQueue::new();
    queue.submit(unit("j1", "magistr")).expect("submitted");
    queue.claim(&fleet, "j1", "n1", at(1)).expect("claimed");

    assert!(fleet.forget("n1"));
    assert_eq!(queue.reclaim(&fleet, at(2)), ["j1"]);
}

#[test]
fn a_wedged_job_on_a_healthy_node_is_reclaimed_by_its_own_lease() {
    // The node keeps heartbeating: a health lease alone would never see this.
    let mut fleet = FleetRegistry::with_lease_ttl_secs(LEASE_TTL);
    fleet.register(node("n1"));
    let mut queue = WorkQueue::with_claim_ttl_secs(30);
    queue.submit(unit("j1", "magistr")).expect("submitted");
    queue.claim(&fleet, "j1", "n1", at(0)).expect("claimed");

    fleet
        .heartbeat("n1", at(40), 0, "")
        .expect("the node is alive and well");
    assert_eq!(
        fleet.node("n1").map(|n| n.health(at(40), LEASE_TTL)),
        Some(NodeHealth::Ready)
    );
    assert_eq!(
        queue.reclaim(&fleet, at(40)),
        ["j1"],
        "the claim lease ran out"
    );
}

#[test]
fn a_reclaimed_jobs_old_holder_is_fenced_out_of_completing_and_reporting() {
    let mut fleet = FleetRegistry::with_lease_ttl_secs(LEASE_TTL);
    fleet.register(node("n1"));
    fleet.register(FleetNode::new("n2", "n2.local", "1.0.0", at(0)));

    let mut queue = WorkQueue::with_claim_ttl_secs(30);
    queue.submit(unit("j1", "magistr")).expect("submitted");
    let stale_attempt = queue.claim(&fleet, "j1", "n1", at(0)).expect("claimed");
    queue
        .report("j1", "n1", stale_attempt, 0, msg("first try"))
        .expect("taken");

    // The claim lease runs out and the job lands somewhere else.
    fleet.heartbeat("n1", at(40), 0, "").expect("alive");
    assert_eq!(queue.reclaim(&fleet, at(40)), ["j1"]);
    let fresh_attempt = queue.claim(&fleet, "j1", "n2", at(40)).expect("re-claimed");
    assert_eq!(fresh_attempt, 2);

    // The zombie finishes and reports. Both are refused.
    let late_event = queue
        .report("j1", "n1", stale_attempt, 1, msg("zombie"))
        .expect_err("fenced");
    assert!(
        late_event.to_string().contains("not held by"),
        "{late_event}"
    );

    let late_done = queue
        .complete("j1", "n1", stale_attempt, true)
        .expect_err("fenced");
    assert!(late_done.to_string().contains("not held by"), "{late_done}");

    // Even the right node cannot present the old attempt.
    let old_number = queue
        .complete("j1", "n2", stale_attempt, true)
        .expect_err("fenced by number");
    assert!(
        old_number.to_string().contains("has been reclaimed"),
        "{old_number}"
    );

    // And the reader's log grows rather than rewinding.
    queue
        .report("j1", "n2", fresh_attempt, 0, msg("second try"))
        .expect("taken");
    assert_eq!(
        queue.events("j1", 0).expect("a known job"),
        [msg("first try"), msg("second try")]
    );
    assert_eq!(
        queue.events("j1", 1).expect("a known job"),
        [msg("second try")]
    );
}

#[test]
fn a_settled_job_is_never_reclaimed() {
    let fleet = fleet(vec![node("n1")]);
    let mut queue = WorkQueue::with_claim_ttl_secs(1);
    queue.submit(unit("j1", "magistr")).expect("submitted");
    let attempt = queue.claim(&fleet, "j1", "n1", at(0)).expect("claimed");
    queue.complete("j1", "n1", attempt, true).expect("settled");

    assert!(queue.reclaim(&fleet, at(9_999)).is_empty());
    assert_eq!(queue.count_in(JobState::Completed), 1);
}

// ── Cordon and drain ─────────────────────────────────────────

fn snapshot_at(phase: crate::workflow::WorkflowPhase, iteration: u32) -> SessionSnapshot {
    let mut session = crate::session::EngineSession::new("sess-migrating");
    session.set_phase(phase);
    session.record_turn(0.5, 100, 20);
    SessionSnapshot::capture(
        &session,
        crate::workflow::IterationState {
            iteration,
            max_iterations: 10,
            last_status: crate::workflow::IterationStatus::None,
            spec_checked: 1,
            spec_total: 4,
        },
        at(0),
    )
}

#[test]
fn a_cordoned_node_takes_no_new_work() {
    let mut fleet = fleet(vec![node("n1")]);
    let mut queue = WorkQueue::new();
    queue.submit(unit("j1", "p")).expect("submitted");

    assert_eq!(queue.free_slots(&fleet, "n1", at(1)), 1);
    fleet.cordon("n1", true).expect("cordoned");
    assert_eq!(queue.free_slots(&fleet, "n1", at(1)), 0);
    assert_eq!(queue.place_on(&fleet, "n1", at(1)), None);
    assert_eq!(queue.place(&fleet, at(1)), None);
    assert!(queue.claim(&fleet, "j1", "n1", at(1)).is_err());
}

#[test]
fn uncordoning_lets_the_node_take_work_again() {
    let mut fleet = fleet(vec![node("n1")]);
    let mut queue = WorkQueue::new();
    queue.submit(unit("j1", "p")).expect("submitted");
    fleet.cordon("n1", true).expect("cordoned");
    assert!(fleet.is_draining("n1"));

    fleet.cordon("n1", false).expect("uncordoned");
    assert!(!fleet.is_draining("n1"));
    assert_eq!(queue.claim(&fleet, "j1", "n1", at(1)), Ok(1));
}

#[test]
fn cordoning_an_unknown_node_is_refused_rather_than_answered() {
    let mut fleet = fleet(vec![node("n1")]);
    assert!(fleet.cordon("ghost", true).is_err());
    // An unknown node takes no work, so it reads as draining.
    assert!(fleet.is_draining("ghost"));
}

#[test]
fn draining_a_node_that_is_not_cordoned_is_refused() {
    let fleet = fleet(vec![node("n1")]);
    let mut queue = WorkQueue::new();
    queue.submit(unit("j1", "p")).expect("submitted");
    queue.claim(&fleet, "j1", "n1", at(1)).expect("claimed");

    let err = queue.drain(&fleet, "n1").expect_err("must cordon first");
    assert!(err.to_string().contains("cordoned"), "{err}");
    assert_eq!(queue.job("j1").expect("job").state, JobState::Claimed);
}

#[test]
fn draining_an_unknown_node_is_refused() {
    let fleet = fleet(vec![node("n1")]);
    let mut queue = WorkQueue::new();
    assert!(queue.drain(&fleet, "ghost").is_err());
}

#[test]
fn draining_hands_the_work_off_rather_than_failing_it() {
    let mut fleet = fleet(vec![node("n1"), node("n2")]);
    let mut queue = WorkQueue::new();
    queue.submit(unit("j1", "p")).expect("submitted");
    queue.submit(unit("j2", "p")).expect("submitted");
    let attempt = queue.claim(&fleet, "j1", "n1", at(1)).expect("claimed");
    assert_eq!(attempt, 1);
    queue
        .checkpoint(
            "j1",
            "n1",
            attempt,
            snapshot_at(crate::workflow::WorkflowPhase::Executing, 3),
        )
        .expect("checkpointed");

    fleet.cordon("n1", true).expect("cordoned");
    let handed_off = queue.drain(&fleet, "n1").expect("drained");
    assert_eq!(handed_off, vec!["j1".to_owned()]);

    let job = queue.job("j1").expect("job");
    assert_eq!(job.state, JobState::Queued, "handed off, not failed");
    assert_eq!(job.node_id, None);
    assert_eq!(job.attempt, 1, "the bump belongs to the next claim");
    let carried = job.snapshot.as_ref().expect("the snapshot came with it");
    assert_eq!(carried.phase, crate::workflow::WorkflowPhase::Executing);
    assert_eq!(carried.iteration.iteration, 3);
}

#[test]
fn a_drained_job_resumes_on_another_node_from_where_it_stopped() {
    let mut fleet = fleet(vec![node("n1"), node("n2")]);
    let mut queue = WorkQueue::new();
    queue.submit(unit("j1", "p")).expect("submitted");
    let attempt = queue.claim(&fleet, "j1", "n1", at(1)).expect("claimed");
    queue
        .checkpoint(
            "j1",
            "n1",
            attempt,
            snapshot_at(crate::workflow::WorkflowPhase::Verifying, 5),
        )
        .expect("checkpointed");

    fleet.cordon("n1", true).expect("cordoned");
    queue.drain(&fleet, "n1").expect("drained");

    // The cordoned node is out of the running, so placement picks n2.
    let (job_id, node_id) = queue.place(&fleet, at(2)).expect("re-placed");
    assert_eq!(job_id, "j1");
    assert_eq!(node_id, "n2");

    let next = queue.claim(&fleet, "j1", "n2", at(2)).expect("re-claimed");
    assert_eq!(next, 2, "the new holder is fenced past the old one");
    let resumed = queue
        .job("j1")
        .and_then(|job| job.snapshot.as_ref())
        .expect("resumes from the checkpoint");
    assert_eq!(resumed.iteration.iteration, 5);
    assert_eq!(resumed.phase, crate::workflow::WorkflowPhase::Verifying);
}

#[test]
fn draining_leaves_other_nodes_jobs_alone() {
    let mut fleet = fleet(vec![node("n1"), node("n2")]);
    let mut queue = WorkQueue::new();
    queue.submit(unit("j1", "p")).expect("submitted");
    queue.submit(unit("j2", "p")).expect("submitted");
    queue.claim(&fleet, "j1", "n1", at(1)).expect("claimed");
    queue.claim(&fleet, "j2", "n2", at(1)).expect("claimed");

    fleet.cordon("n1", true).expect("cordoned");
    assert_eq!(queue.drain(&fleet, "n1").expect("drained"), vec!["j1"]);
    assert_eq!(queue.job("j2").expect("job").state, JobState::Claimed);
}

#[test]
fn draining_a_node_holding_nothing_is_an_empty_hand_off() {
    let mut fleet = fleet(vec![node("n1")]);
    let mut queue = WorkQueue::new();
    fleet.cordon("n1", true).expect("cordoned");
    assert!(queue.drain(&fleet, "n1").expect("drained").is_empty());
}

#[test]
fn a_settled_job_is_not_handed_off_by_a_drain() {
    let mut fleet = fleet(vec![node("n1")]);
    let mut queue = WorkQueue::new();
    queue.submit(unit("j1", "p")).expect("submitted");
    let attempt = queue.claim(&fleet, "j1", "n1", at(1)).expect("claimed");
    queue
        .complete("j1", "n1", attempt, true)
        .expect("completed");

    fleet.cordon("n1", true).expect("cordoned");
    assert!(queue.drain(&fleet, "n1").expect("drained").is_empty());
    assert_eq!(queue.job("j1").expect("job").state, JobState::Completed);
}

// ── Checkpoint fencing ──────────────────────────────────────────────

#[test]
fn a_checkpoint_from_the_holder_replaces_the_previous_one() {
    let fleet = fleet(vec![node("n1")]);
    let mut queue = WorkQueue::new();
    queue.submit(unit("j1", "p")).expect("submitted");
    let attempt = queue.claim(&fleet, "j1", "n1", at(1)).expect("claimed");

    queue
        .checkpoint(
            "j1",
            "n1",
            attempt,
            snapshot_at(crate::workflow::WorkflowPhase::Planning, 1),
        )
        .expect("checkpointed");
    queue
        .checkpoint(
            "j1",
            "n1",
            attempt,
            snapshot_at(crate::workflow::WorkflowPhase::Executing, 2),
        )
        .expect("checkpointed again");

    let carried = queue
        .job("j1")
        .and_then(|job| job.snapshot.as_ref())
        .expect("a snapshot");
    assert_eq!(carried.iteration.iteration, 2, "the newest level wins");
}

#[test]
fn a_checkpoint_from_a_fenced_out_holder_is_refused() {
    let mut fleet = fleet(vec![node("n1"), node("n2")]);
    let mut queue = WorkQueue::new();
    queue.submit(unit("j1", "p")).expect("submitted");
    let old = queue.claim(&fleet, "j1", "n1", at(1)).expect("claimed");
    queue
        .checkpoint(
            "j1",
            "n1",
            old,
            snapshot_at(crate::workflow::WorkflowPhase::Planning, 1),
        )
        .expect("checkpointed");

    fleet.cordon("n1", true).expect("cordoned");
    queue.drain(&fleet, "n1").expect("drained");
    let new = queue.claim(&fleet, "j1", "n2", at(2)).expect("re-claimed");
    queue
        .checkpoint(
            "j1",
            "n2",
            new,
            snapshot_at(crate::workflow::WorkflowPhase::Verifying, 4),
        )
        .expect("the new holder checkpoints");

    // The evicted node finishes its turn and reports the state it had.
    let err = queue
        .checkpoint(
            "j1",
            "n1",
            old,
            snapshot_at(crate::workflow::WorkflowPhase::Planning, 1),
        )
        .expect_err("the old holder is fenced out");
    assert!(err.to_string().contains("not held by node 'n1'"), "{err}");

    let carried = queue
        .job("j1")
        .and_then(|job| job.snapshot.as_ref())
        .expect("a snapshot");
    assert_eq!(
        carried.iteration.iteration, 4,
        "the zombie did not overwrite the new holder's state"
    );
}

#[test]
fn a_stale_attempt_is_refused_even_when_the_same_node_holds_the_job_again() {
    // The holder check catches a job that moved machines; the attempt is
    // what catches a job that came back to the same one.
    let mut fleet = fleet(vec![node("n1")]);
    let mut queue = WorkQueue::new();
    queue.submit(unit("j1", "p")).expect("submitted");
    let old = queue.claim(&fleet, "j1", "n1", at(1)).expect("claimed");

    fleet.cordon("n1", true).expect("cordoned");
    queue.drain(&fleet, "n1").expect("drained");
    fleet.cordon("n1", false).expect("uncordoned");
    let new = queue.claim(&fleet, "j1", "n1", at(2)).expect("re-claimed");
    assert_eq!((old, new), (1, 2));

    let err = queue
        .checkpoint(
            "j1",
            "n1",
            old,
            snapshot_at(crate::workflow::WorkflowPhase::Planning, 1),
        )
        .expect_err("the previous attempt is fenced out");
    assert!(err.to_string().contains("attempt"), "{err}");
}

#[test]
fn a_checkpoint_for_a_job_no_node_holds_is_refused() {
    let mut queue = WorkQueue::new();
    queue.submit(unit("j1", "p")).expect("submitted");
    assert!(
        queue
            .checkpoint(
                "j1",
                "n1",
                1,
                snapshot_at(crate::workflow::WorkflowPhase::Planning, 0)
            )
            .is_err()
    );
    assert!(
        queue
            .checkpoint(
                "ghost",
                "n1",
                1,
                snapshot_at(crate::workflow::WorkflowPhase::Planning, 0)
            )
            .is_err()
    );
}
