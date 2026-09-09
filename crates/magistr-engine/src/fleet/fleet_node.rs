use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::node_health::NodeHealth;
use super::policy_bundle::PolicyBundle;

/// One machine in the fleet, as the control plane sees it.
///
/// A node is identified by an id it chooses and keeps across restarts;
/// everything else is what it last told the control plane. `last_seen`
/// is the lease — it is renewed by a heartbeat and nothing else, so the
/// control plane never has to trust a node's opinion of its own health.
///
/// `applied_generation` and `applied_digest` are the node's claim about
/// which [`PolicyBundle`] it is actually running. Keeping both lets a
/// node that reports the right generation with the wrong content be
/// counted as drifted rather than current.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetNode {
    /// Stable id the node keeps across restarts.
    pub id: String,
    /// Host the node runs on, for operator legibility.
    pub hostname: String,
    /// The magistr version the node runs.
    pub version: String,
    /// Project ids this node serves.
    #[serde(default)]
    pub projects: Vec<String>,
    /// When the lease was last renewed.
    pub last_seen: DateTime<Utc>,
    /// The bundle generation the node claims to be running.
    #[serde(default)]
    pub applied_generation: u64,
    /// The bundle digest the node claims to be running.
    #[serde(default)]
    pub applied_digest: String,
    /// How many units of work the node will hold at once.
    ///
    /// Declared by the node, but never *counted* by it: how many jobs
    /// it actually holds is derived from the queue
    /// ([`WorkQueue::in_flight`]), so there is no per-node counter for a
    /// lost release to leave wrong. A record written before capacity
    /// existed reads as a single slot — the conservative direction, and
    /// a node that can do more says so by registering again.
    ///
    /// [`WorkQueue::in_flight`]: super::work_queue::WorkQueue::in_flight
    #[serde(default = "default_capacity")]
    pub capacity: u32,
    /// Whether an operator has cordoned this node off.
    ///
    /// The one *asserted* fact in a record whose every other fact is
    /// derived — and it is asserted because it is an intent rather than
    /// an observation. Health cannot be declared (a dying machine would
    /// declare itself well); willingness to take new work cannot be
    /// derived (a healthy machine about to be rebooted looks exactly
    /// like one that is not). So the operator says it, and everything
    /// downstream — [`accepts_work`], [`WorkQueue::free_slots`],
    /// eligibility to be drained — is derived from it.
    ///
    /// A record written before draining existed reads as not cordoned,
    /// which is the conservative direction: it keeps taking work.
    ///
    /// [`accepts_work`]: FleetNode::accepts_work
    /// [`WorkQueue::free_slots`]: super::work_queue::WorkQueue::free_slots
    #[serde(default)]
    pub draining: bool,
}

/// What a node's capacity is when it has not said.
fn default_capacity() -> u32 {
    FleetNode::DEFAULT_CAPACITY
}

impl FleetNode {
    /// Units of work a node holds at once unless it declares otherwise.
    pub const DEFAULT_CAPACITY: u32 = 1;

    /// A node joining the fleet at `now`, running no policy yet.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        hostname: impl Into<String>,
        version: impl Into<String>,
        now: DateTime<Utc>,
    ) -> Self {
        Self {
            id: id.into(),
            hostname: hostname.into(),
            version: version.into(),
            projects: Vec::new(),
            last_seen: now,
            applied_generation: 0,
            applied_digest: String::new(),
            capacity: Self::DEFAULT_CAPACITY,
            draining: false,
        }
    }

    /// Set the projects this node serves (self-consuming builder).
    #[must_use]
    pub fn with_projects(mut self, projects: Vec<String>) -> Self {
        self.projects = projects;
        self
    }

    /// Set how many units of work the node will hold at once.
    #[must_use]
    pub fn with_capacity(mut self, capacity: u32) -> Self {
        self.capacity = capacity;
        self
    }

    /// Stop the node taking new work — the cordon half of a drain.
    ///
    /// Idempotent, and deliberately says nothing about the work the node
    /// already holds: cordoning is what makes the eviction safe, and
    /// doing both in one verb would leave no way to cordon a node you
    /// merely want to quiesce.
    pub fn cordon(&mut self) {
        self.draining = true;
    }

    /// Let the node take work again.
    pub fn uncordon(&mut self) {
        self.draining = false;
    }

    /// Whether the node will accept new work at `now`.
    ///
    /// Ready *and* not cordoned. Health alone is not enough: a node
    /// being drained for maintenance is perfectly healthy right up to
    /// the moment it is switched off, which is exactly why the cordon
    /// exists.
    #[must_use]
    pub fn accepts_work(&self, now: DateTime<Utc>, lease_ttl_secs: i64) -> bool {
        !self.draining && self.health(now, lease_ttl_secs) == NodeHealth::Ready
    }

    /// Whether the node is willing to run work for `project`.
    ///
    /// An empty `projects` list means the node is general-purpose.
    /// [`FleetNode::new`] starts empty and [`with_projects`] narrows, so
    /// the field reads as a *restriction* a node opts into rather than a
    /// capability list it must remember to fill in — and a fleet whose
    /// operators never labelled anything still schedules.
    ///
    /// [`with_projects`]: FleetNode::with_projects
    #[must_use]
    pub fn serves(&self, project: &str) -> bool {
        self.projects.is_empty() || self.projects.iter().any(|p| p == project)
    }

    /// Renew the lease and record what the node is running.
    pub fn renew(
        &mut self,
        now: DateTime<Utc>,
        applied_generation: u64,
        applied_digest: impl Into<String>,
    ) {
        self.last_seen = now;
        self.applied_generation = applied_generation;
        self.applied_digest = applied_digest.into();
    }

    /// Seconds since the lease was last renewed.
    #[must_use]
    pub fn lease_age_secs(&self, now: DateTime<Utc>) -> i64 {
        now.signed_duration_since(self.last_seen).num_seconds()
    }

    /// The node's health at `now`, given the fleet's lease TTL.
    #[must_use]
    pub fn health(&self, now: DateTime<Utc>, lease_ttl_secs: i64) -> NodeHealth {
        NodeHealth::from_lease(self.lease_age_secs(now), lease_ttl_secs)
    }

    /// Whether the node is running exactly `bundle`.
    ///
    /// Both halves are checked: the generation says *which* policy the
    /// node thinks it has, the digest says whether that policy is the
    /// one the control plane published under that number.
    #[must_use]
    pub fn is_current(&self, bundle: &PolicyBundle) -> bool {
        self.applied_generation == bundle.generation && self.applied_digest == bundle.digest
    }
}

#[cfg(test)]
#[path = "fleet_node.tests.rs"]
mod tests;
