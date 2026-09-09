use chrono::{DateTime, Utc};

use crate::session::SessionSnapshot;

use super::job_state::JobState;
use super::stream_relay::StreamRelay;
use super::work_unit::WorkUnit;

/// A [`WorkUnit`] plus everything the control plane knows about how its
/// execution is going: who holds it, since when, how many times it has
/// been handed out, and the ordered stream it has produced.
///
/// `attempt` is the load-bearing field. It starts at zero, is returned
/// by a claim, and is demanded back by every later message about the
/// job — a completion, a failure, an event. Reclaiming the job bumps it,
/// which is what makes a reclaim safe: the node that lost the job can
/// still be alive, still finishing, and still about to report, and every
/// one of those messages now names an attempt that has passed. The
/// counter is therefore both the retry count an operator reads and the
/// fencing token that keeps a zombie out, and there is no second field
/// for the two of them to disagree through.
///
/// `claimed_at` is a lease, in the same sense [`FleetNode::last_seen`]
/// is: it is not renewed by anything the node says about its progress,
/// only by taking the claim. So a node that wedges without dying still
/// loses the job, which is the case a health lease alone cannot see.
///
/// [`FleetNode::last_seen`]: super::fleet_node::FleetNode::last_seen
#[derive(Debug, Clone)]
pub struct Job {
    /// What is to be done.
    pub unit: WorkUnit,
    /// Where the job is in its lifecycle.
    pub state: JobState,
    /// The node holding it, while it is claimed.
    pub node_id: Option<String>,
    /// When the current claim was taken.
    pub claimed_at: Option<DateTime<Utc>>,
    /// How many times the job has been handed to a node.
    pub attempt: u32,
    /// The ordered event stream this job has produced.
    pub relay: StreamRelay,
    /// The last session snapshot the holder reported, if any.
    ///
    /// This is what makes a hand-off a hand-off rather than a restart.
    /// It survives [`requeue`] and [`claim`] for the same reason
    /// [`StreamRelay::delivered`] does: the state a node checkpointed is
    /// a fact about the *job*, not about the attempt that produced it,
    /// so a job evicted from a draining node and re-placed elsewhere
    /// resumes from it rather than starting the work again.
    ///
    /// [`requeue`]: Job::requeue
    /// [`claim`]: Job::claim
    /// [`StreamRelay::delivered`]: super::stream_relay::StreamRelay::delivered
    pub snapshot: Option<SessionSnapshot>,
}

impl Job {
    /// A freshly submitted job, queued and never yet claimed.
    #[must_use]
    pub fn new(unit: WorkUnit) -> Self {
        Self {
            unit,
            state: JobState::Queued,
            node_id: None,
            claimed_at: None,
            attempt: 0,
            relay: StreamRelay::new(),
            snapshot: None,
        }
    }

    /// The job's id.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.unit.id
    }

    /// Whether `node_id` is the node currently holding this job.
    #[must_use]
    pub fn held_by(&self, node_id: &str) -> bool {
        self.node_id.as_deref() == Some(node_id)
    }

    /// Seconds since the current claim was taken, or `None` when the job
    /// is not claimed.
    #[must_use]
    pub fn claim_age_secs(&self, now: DateTime<Utc>) -> Option<i64> {
        self.claimed_at
            .map(|at| now.signed_duration_since(at).num_seconds())
    }

    /// Whether the claim lease has run out at `now`.
    ///
    /// A negative age — a control plane whose clock has stepped back —
    /// reads as a fresh claim rather than an expired one, the same
    /// choice [`NodeHealth::from_lease`] makes for the same reason.
    ///
    /// [`NodeHealth::from_lease`]: super::node_health::NodeHealth::from_lease
    #[must_use]
    pub fn is_lease_expired(&self, now: DateTime<Utc>, claim_ttl_secs: i64) -> bool {
        self.state == JobState::Claimed
            && self
                .claim_age_secs(now)
                .is_some_and(|age| age > claim_ttl_secs.max(1))
    }

    /// Hand the job to a node, returning the attempt number the node
    /// must present with everything it later reports.
    pub fn claim(&mut self, node_id: impl Into<String>, now: DateTime<Utc>) -> u32 {
        self.state = JobState::Claimed;
        self.node_id = Some(node_id.into());
        self.claimed_at = Some(now);
        self.attempt += 1;
        self.relay.restart(self.attempt);
        self.attempt
    }

    /// Put the job back on the queue, leaving its attempt count alone.
    ///
    /// The bump belongs to [`claim`], not here: counting on release
    /// would leave a job that was reclaimed and never re-placed
    /// advertising an attempt no node ever ran.
    ///
    /// [`claim`]: Job::claim
    pub fn requeue(&mut self) {
        self.state = JobState::Queued;
        self.node_id = None;
        self.claimed_at = None;
    }

    /// Record the state the holder checkpointed.
    ///
    /// Replaces whatever was there: a checkpoint is a *level*, the
    /// session as it stands, not an increment — the newest one is the
    /// only one worth resuming from.
    pub fn checkpoint(&mut self, snapshot: SessionSnapshot) {
        self.snapshot = Some(snapshot);
    }

    /// Record the outcome a node reported.
    pub fn settle(&mut self, succeeded: bool) {
        self.state = if succeeded {
            JobState::Completed
        } else {
            JobState::Failed
        };
        self.claimed_at = None;
    }
}

#[cfg(test)]
#[path = "job.tests.rs"]
mod tests;
