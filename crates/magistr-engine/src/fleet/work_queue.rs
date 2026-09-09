use std::collections::BTreeMap;

use chrono::{DateTime, Utc};

use crate::error::EngineError;
use crate::session::{SessionEvent, SessionSnapshot};

use super::fleet_registry::FleetRegistry;
use super::job::Job;
use super::job_state::JobState;
use super::node_health::NodeHealth;
use super::work_unit::WorkUnit;

/// Queued jobs, their leases, and their ordered event streams.
///
/// Scheduling methods consult the supplied [`FleetRegistry`] directly.
/// Capacity is counted from in-flight jobs; [`place`] selects a Ready node
/// that serves the project and has a free slot.
///
/// [`claim`] returns an attempt number required by [`complete`] and [`report`].
/// [`reclaim`] requeues jobs when a claim expires or its node becomes
/// unreachable. The next claim increments the attempt, rejecting late results
/// from the previous holder. A slow node may still execute a job twice;
/// [`StreamRelay`] preserves the event history across attempts.
///
/// [`drain`] requires a cordoned node before eviction so freed jobs cannot be
/// placed straight back onto it. Requeued jobs retain their last checkpoint
/// for the next holder to resume.
///
/// [`StreamRelay`]: super::stream_relay::StreamRelay
/// [`place`]: WorkQueue::place
/// [`claim`]: WorkQueue::claim
/// [`complete`]: WorkQueue::complete
/// [`report`]: WorkQueue::report
/// [`reclaim`]: WorkQueue::reclaim
/// [`drain`]: WorkQueue::drain
#[derive(Debug, Clone)]
pub struct WorkQueue {
    claim_ttl_secs: i64,
    jobs: BTreeMap<String, Job>,
    order: Vec<String>,
}

impl WorkQueue {
    /// How long a claim holds a job without being renewed, by default.
    ///
    /// Longer than [`FleetRegistry::DEFAULT_LEASE_TTL_SECS`] by design:
    /// an agent turn routinely outlives a heartbeat interval, so the
    /// health lease is the fast detector of a dead machine and the claim
    /// lease is the slow detector of a wedged job.
    pub const DEFAULT_CLAIM_TTL_SECS: i64 = 900;

    /// An empty queue with the default claim TTL.
    #[must_use]
    pub fn new() -> Self {
        Self::with_claim_ttl_secs(Self::DEFAULT_CLAIM_TTL_SECS)
    }

    /// An empty queue with a custom claim TTL (floored at one second).
    #[must_use]
    pub fn with_claim_ttl_secs(claim_ttl_secs: i64) -> Self {
        Self {
            claim_ttl_secs: claim_ttl_secs.max(1),
            jobs: BTreeMap::new(),
            order: Vec::new(),
        }
    }

    /// How long a claim holds a job.
    #[must_use]
    pub fn claim_ttl_secs(&self) -> i64 {
        self.claim_ttl_secs
    }

    // ── The ledger ──────────────────────────────────────────────────

    /// Take a new unit of work. A duplicate id is refused rather than
    /// replacing the job already under it — that is what makes a retried
    /// submission idempotent.
    pub fn submit(&mut self, unit: WorkUnit) -> Result<&Job, EngineError> {
        if self.jobs.contains_key(&unit.id) {
            return Err(EngineError::Precondition(format!(
                "job '{}' has already been submitted",
                unit.id
            )));
        }
        let id = unit.id.clone();
        self.order.push(id.clone());
        Ok(self.jobs.entry(id).or_insert_with(|| Job::new(unit)))
    }

    /// Look up one job.
    #[must_use]
    pub fn job(&self, id: &str) -> Option<&Job> {
        self.jobs.get(id)
    }

    /// Every job, in submission order.
    #[must_use]
    pub fn jobs(&self) -> Vec<&Job> {
        self.order
            .iter()
            .filter_map(|id| self.jobs.get(id))
            .collect()
    }

    /// How many jobs the queue holds, in any state.
    #[must_use]
    pub fn len(&self) -> usize {
        self.jobs.len()
    }

    /// Whether the queue holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }

    /// How many jobs are in `state`.
    #[must_use]
    pub fn count_in(&self, state: JobState) -> usize {
        self.jobs.values().filter(|job| job.state == state).count()
    }

    /// Drop a job an operator has retired, and its event log with it.
    /// Returns whether it existed.
    pub fn forget(&mut self, id: &str) -> bool {
        let existed = self.jobs.remove(id).is_some();
        if existed {
            self.order.retain(|known| known != id);
        }
        existed
    }

    // ── Capacity ────────────────────────────────────────────────────

    /// How many jobs `node_id` is currently holding — counted from the
    /// jobs themselves, never from a tally the node keeps.
    #[must_use]
    pub fn in_flight(&self, node_id: &str) -> u32 {
        u32::try_from(
            self.jobs
                .values()
                .filter(|job| job.state == JobState::Claimed && job.held_by(node_id))
                .count(),
        )
        .unwrap_or(u32::MAX)
    }

    /// How many more jobs `node_id` will take, or zero when the node is
    /// unknown, not Ready, or already full.
    #[must_use]
    pub fn free_slots(&self, fleet: &FleetRegistry, node_id: &str, now: DateTime<Utc>) -> u32 {
        let Some(node) = fleet.node(node_id) else {
            return 0;
        };
        if !node.accepts_work(now, fleet.lease_ttl_secs()) {
            return 0;
        }
        node.capacity.saturating_sub(self.in_flight(node_id))
    }

    // ── Scheduling ──────────────────────────────────────────────────

    /// The next job `node_id` may take, or `None` when there is nothing
    /// for it. Pure: it decides, it does not claim.
    #[must_use]
    pub fn place_on(
        &self,
        fleet: &FleetRegistry,
        node_id: &str,
        now: DateTime<Utc>,
    ) -> Option<String> {
        if self.free_slots(fleet, node_id, now) == 0 {
            return None;
        }
        let node = fleet.node(node_id)?;
        self.order
            .iter()
            .filter_map(|id| self.jobs.get(id))
            .find(|job| job.state == JobState::Queued && node.serves(&job.unit.project))
            .map(|job| job.unit.id.clone())
    }

    /// The next (job, node) pair the fleet should run, or `None` when
    /// nothing queued can be placed anywhere right now. Pure.
    ///
    /// The oldest placeable job wins; among the nodes that could take it
    /// the one with the most free slots wins, and ties break on the
    /// lower node id so the answer never depends on iteration order.
    #[must_use]
    pub fn place(&self, fleet: &FleetRegistry, now: DateTime<Utc>) -> Option<(String, String)> {
        self.order
            .iter()
            .filter_map(|id| self.jobs.get(id))
            .filter(|job| job.state == JobState::Queued)
            .find_map(|job| {
                let (_, node_id) = fleet
                    .nodes()
                    .into_iter()
                    .filter(|node| node.serves(&job.unit.project))
                    .map(|node| (self.free_slots(fleet, &node.id, now), node.id.clone()))
                    .filter(|(free, _)| *free > 0)
                    .max_by(|(free_a, id_a), (free_b, id_b)| {
                        free_a.cmp(free_b).then_with(|| id_b.cmp(id_a))
                    })?;
                Some((job.unit.id.clone(), node_id))
            })
    }

    /// Hand `job_id` to `node_id`, returning the attempt number the node
    /// must present with everything it later reports.
    ///
    /// The eligibility rule lives here rather than only in [`place`], so
    /// a caller that picked a node some other way — a wire request, a
    /// test — cannot put work on a node that is not Ready, does not
    /// serve the project, or is already full.
    ///
    /// [`place`]: WorkQueue::place
    pub fn claim(
        &mut self,
        fleet: &FleetRegistry,
        job_id: &str,
        node_id: &str,
        now: DateTime<Utc>,
    ) -> Result<u32, EngineError> {
        let project = {
            let job = self.require(job_id)?;
            if job.state != JobState::Queued {
                return Err(EngineError::Precondition(format!(
                    "job '{job_id}' is {} and cannot be claimed",
                    job.state.as_str()
                )));
            }
            job.unit.project.clone()
        };

        let Some(node) = fleet.node(node_id) else {
            return Err(EngineError::Precondition(format!(
                "node '{node_id}' is not registered with this fleet"
            )));
        };
        if node.draining {
            return Err(EngineError::Precondition(format!(
                "node '{node_id}' is draining and takes no new work"
            )));
        }
        let health = node.health(now, fleet.lease_ttl_secs());
        if health != NodeHealth::Ready {
            return Err(EngineError::Precondition(format!(
                "node '{node_id}' is {} and takes no new work",
                health.as_str()
            )));
        }
        if !node.serves(&project) {
            return Err(EngineError::Precondition(format!(
                "node '{node_id}' does not serve project '{project}'"
            )));
        }
        if self.free_slots(fleet, node_id, now) == 0 {
            return Err(EngineError::Precondition(format!(
                "node '{node_id}' is at its capacity of {}",
                node.capacity
            )));
        }

        let job = self
            .jobs
            .get_mut(job_id)
            .ok_or_else(|| Self::no_such_job(job_id))?;
        Ok(job.claim(node_id, now))
    }

    /// Claim the next job `node_id` may take, if any — the pull half of
    /// scheduling, for a node asking the control plane for work.
    pub fn claim_next(
        &mut self,
        fleet: &FleetRegistry,
        node_id: &str,
        now: DateTime<Utc>,
    ) -> Result<Option<(String, u32)>, EngineError> {
        let Some(job_id) = self.place_on(fleet, node_id, now) else {
            return Ok(None);
        };
        let attempt = self.claim(fleet, &job_id, node_id, now)?;
        Ok(Some((job_id, attempt)))
    }

    /// Place as much queued work as the fleet has room for, returning
    /// the (job, node) pairs in the order they were placed — the push
    /// half of scheduling, for a control plane driving the fleet.
    ///
    /// Terminates because every iteration either claims a job (removing
    /// it from the queued set and consuming a slot) or [`place`] answers
    /// `None`.
    ///
    /// [`place`]: WorkQueue::place
    pub fn dispatch(&mut self, fleet: &FleetRegistry, now: DateTime<Utc>) -> Vec<(String, String)> {
        let mut placed = Vec::new();
        while let Some((job_id, node_id)) = self.place(fleet, now) {
            if self.claim(fleet, &job_id, &node_id, now).is_err() {
                break;
            }
            placed.push((job_id, node_id));
        }
        placed
    }

    // ── Reporting ───────────────────────────────────────────────────

    /// Take one session event a node produced for a job it holds.
    ///
    /// Returns how many events became deliverable to the client, which
    /// is zero for a repeat and more than one when the event closed a
    /// gap. Refused when the node is not the holder, or when it names an
    /// attempt the job has moved past.
    pub fn report(
        &mut self,
        job_id: &str,
        node_id: &str,
        attempt: u32,
        seq: u64,
        event: SessionEvent,
    ) -> Result<usize, EngineError> {
        let job = self.require_held(job_id, node_id, attempt)?;
        job.relay.accept(attempt, seq, event)
    }

    /// Take the session state a node checkpointed for a job it holds.
    ///
    /// Fenced exactly like [`report`]: a node that lost the job cannot
    /// overwrite the snapshot the new holder is working from with the
    /// state it had when it was evicted.
    ///
    /// [`report`]: WorkQueue::report
    pub fn checkpoint(
        &mut self,
        job_id: &str,
        node_id: &str,
        attempt: u32,
        snapshot: SessionSnapshot,
    ) -> Result<(), EngineError> {
        let job = self.require_held(job_id, node_id, attempt)?;
        job.checkpoint(snapshot);
        Ok(())
    }

    /// Record the outcome a node reported for a job it holds, releasing
    /// the claim. Refused on the same fence [`report`] uses.
    ///
    /// [`report`]: WorkQueue::report
    pub fn complete(
        &mut self,
        job_id: &str,
        node_id: &str,
        attempt: u32,
        succeeded: bool,
    ) -> Result<JobState, EngineError> {
        let job = self.require_held(job_id, node_id, attempt)?;
        job.settle(succeeded);
        Ok(job.state)
    }

    /// The events a client has not read yet, given its cursor.
    #[must_use]
    pub fn events(&self, job_id: &str, cursor: usize) -> Option<&[SessionEvent]> {
        self.jobs.get(job_id).map(|job| job.relay.since(cursor))
    }

    // ── Reclaim ─────────────────────────────────────────────────────

    /// Re-queue every claimed job whose holder has gone — because the
    /// node is unreachable or unregistered, or because the claim lease
    /// ran out under it — and return their ids in submission order.
    ///
    /// Nothing is bumped here: the attempt advances on the *next* claim,
    /// so a job reclaimed and never re-placed does not advertise an
    /// attempt no node ever ran.
    pub fn reclaim(&mut self, fleet: &FleetRegistry, now: DateTime<Utc>) -> Vec<String> {
        let ttl = fleet.lease_ttl_secs();
        let claim_ttl = self.claim_ttl_secs;
        let mut reclaimed = Vec::new();

        for id in &self.order {
            let Some(job) = self.jobs.get_mut(id) else {
                continue;
            };
            if job.state != JobState::Claimed {
                continue;
            }
            let holder_gone = job
                .node_id
                .as_deref()
                .and_then(|node_id| fleet.node(node_id))
                .is_none_or(|node| !node.health(now, ttl).is_live());

            if holder_gone || job.is_lease_expired(now, claim_ttl) {
                job.requeue();
                reclaimed.push(id.clone());
            }
        }
        reclaimed
    }

    // ── Drain ───────────────────────────────────────────────────────

    /// Hand a cordoned node's work back to the queue, returning the job
    /// ids in submission order.
    ///
    /// The eviction half of a drain, and it *refuses a node that is not
    /// cordoned*. That is not politeness: [`free_slots`] is what makes a
    /// cordon real, so evicting first would re-place the very jobs just
    /// freed onto the machine they were freed from — the drain would
    /// churn the attempt counter and change nothing. Cordon, then drain,
    /// is the order every cluster manager settled on, and putting the
    /// check here rather than in a caller means a wire request cannot do
    /// it the other way round.
    ///
    /// A drained job goes back to [`JobState::Queued`], not `Failed`: it
    /// did not fail, its machine went away. It keeps the snapshot its
    /// holder last checkpointed, so the node that picks it up resumes
    /// rather than starting over, and the attempt bump that fences the
    /// old holder out happens on the *next* claim exactly as it does
    /// for [`reclaim`].
    ///
    /// [`free_slots`]: WorkQueue::free_slots
    /// [`reclaim`]: WorkQueue::reclaim
    pub fn drain(
        &mut self,
        fleet: &FleetRegistry,
        node_id: &str,
    ) -> Result<Vec<String>, EngineError> {
        let Some(node) = fleet.node(node_id) else {
            return Err(EngineError::Precondition(format!(
                "node '{node_id}' is not registered with this fleet"
            )));
        };
        if !node.draining {
            return Err(EngineError::Precondition(format!(
                "node '{node_id}' must be cordoned before it is drained, \
                 or its work would be placed straight back on it"
            )));
        }

        let mut handed_off = Vec::new();
        for id in &self.order {
            let Some(job) = self.jobs.get_mut(id) else {
                continue;
            };
            if job.state == JobState::Claimed && job.held_by(node_id) {
                job.requeue();
                handed_off.push(id.clone());
            }
        }
        Ok(handed_off)
    }

    // ── Helpers ─────────────────────────────────────────────────────

    fn no_such_job(job_id: &str) -> EngineError {
        EngineError::Precondition(format!("no job '{job_id}' in this queue"))
    }

    fn require(&self, job_id: &str) -> Result<&Job, EngineError> {
        self.jobs
            .get(job_id)
            .ok_or_else(|| Self::no_such_job(job_id))
    }

    /// The job, if `node_id` holds it at exactly `attempt`.
    fn require_held(
        &mut self,
        job_id: &str,
        node_id: &str,
        attempt: u32,
    ) -> Result<&mut Job, EngineError> {
        let job = self
            .jobs
            .get_mut(job_id)
            .ok_or_else(|| Self::no_such_job(job_id))?;

        if job.state != JobState::Claimed || !job.held_by(node_id) {
            return Err(EngineError::Precondition(format!(
                "job '{job_id}' is not held by node '{node_id}'"
            )));
        }
        if job.attempt != attempt {
            return Err(EngineError::Precondition(format!(
                "job '{job_id}' is on attempt {} — attempt {attempt} has been reclaimed",
                job.attempt
            )));
        }
        Ok(job)
    }
}

impl Default for WorkQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "work_queue.tests.rs"]
mod tests;
