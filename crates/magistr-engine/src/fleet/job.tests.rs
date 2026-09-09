use super::*;

use chrono::TimeZone;

fn at(secs: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(1_800_000_000 + secs, 0)
        .single()
        .expect("a real instant")
}

fn job() -> Job {
    Job::new(WorkUnit::new("j1", "magistr", "ship it"))
}

#[test]
fn a_submitted_job_is_queued_and_unattempted() {
    let job = job();
    assert_eq!(job.id(), "j1");
    assert_eq!(job.state, JobState::Queued);
    assert_eq!(job.attempt, 0);
    assert!(job.node_id.is_none());
    assert!(job.claim_age_secs(at(0)).is_none());
}

#[test]
fn a_claim_numbers_the_attempt_and_restarts_the_relay_on_it() {
    let mut job = job();
    assert_eq!(job.claim("node-a", at(0)), 1);
    assert_eq!(job.state, JobState::Claimed);
    assert!(job.held_by("node-a"));
    assert!(!job.held_by("node-b"));
    assert_eq!(job.relay.attempt(), 1);
    assert_eq!(job.claim_age_secs(at(5)), Some(5));
}

#[test]
fn a_requeue_leaves_the_attempt_count_where_the_claim_put_it() {
    let mut job = job();
    job.claim("node-a", at(0));
    job.requeue();
    assert_eq!(job.state, JobState::Queued);
    assert_eq!(
        job.attempt, 1,
        "the bump belongs to the claim, not the release"
    );
    assert!(job.node_id.is_none());
    assert!(job.claimed_at.is_none());

    assert_eq!(job.claim("node-b", at(10)), 2);
    assert_eq!(job.relay.attempt(), 2);
}

#[test]
fn the_claim_lease_expires_on_its_own_clock() {
    let mut job = job();
    assert!(
        !job.is_lease_expired(at(0), 30),
        "an unclaimed job cannot expire"
    );

    job.claim("node-a", at(0));
    assert!(
        !job.is_lease_expired(at(30), 30),
        "exactly at the TTL is still held"
    );
    assert!(job.is_lease_expired(at(31), 30));
}

#[test]
fn a_backwards_clock_does_not_expire_a_fresh_claim() {
    let mut job = job();
    job.claim("node-a", at(100));
    assert!(!job.is_lease_expired(at(40), 30));
}

#[test]
fn a_zero_ttl_is_floored_rather_than_expiring_every_claim_instantly() {
    let mut job = job();
    job.claim("node-a", at(0));
    assert!(!job.is_lease_expired(at(0), 0));
    assert!(job.is_lease_expired(at(2), 0));
}

#[test]
fn settling_records_the_outcome_and_releases_the_lease() {
    let mut done = job();
    done.claim("node-a", at(0));
    done.settle(true);
    assert_eq!(done.state, JobState::Completed);
    assert!(done.claimed_at.is_none());
    assert!(
        !done.is_lease_expired(at(9_999), 1),
        "a settled job has no lease left to run out"
    );

    let mut failed = job();
    failed.claim("node-a", at(0));
    failed.settle(false);
    assert_eq!(failed.state, JobState::Failed);
}
