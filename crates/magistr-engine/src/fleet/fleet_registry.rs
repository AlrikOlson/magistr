use std::collections::BTreeMap;

use chrono::{DateTime, Utc};

use crate::error::EngineError;

use super::fleet_node::FleetNode;
use super::fleet_status::FleetStatus;
use super::node_health::NodeHealth;
use super::policy_bundle::PolicyBundle;

/// The control plane's view of the fleet: who is out there, how alive
/// each one is, and which generation of policy each is running.
///
/// The registry owns the published [`PolicyBundle`] as well as the
/// nodes, because the two questions are one question — "is this node
/// current?" has no answer without knowing what current *is*, and
/// keeping them in one place means there is no second copy of the
/// generation to fall behind.
///
/// Two rules make the arithmetic safe. [`publish`] accepts only a
/// strictly newer generation with a digest that matches its contents,
/// so a replayed or mangled bundle cannot roll the fleet back to a
/// looser policy. [`heartbeat`] refuses an id it has never seen instead
/// of auto-registering it, so a node appears in the fleet exactly once,
/// through the front door, with the hostname and version it declared.
///
/// The honest limit: nothing here can *make* a node apply anything. What
/// it buys is that drift is visible, and that whatever a node applies
/// can only tighten it — see [`PolicyBundle::merge_local`].
///
/// [`publish`]: FleetRegistry::publish
/// [`heartbeat`]: FleetRegistry::heartbeat
#[derive(Debug, Clone)]
pub struct FleetRegistry {
    lease_ttl_secs: i64,
    nodes: BTreeMap<String, FleetNode>,
    bundle: PolicyBundle,
}

impl FleetRegistry {
    /// Lease TTL a fleet uses unless an operator says otherwise.
    pub const DEFAULT_LEASE_TTL_SECS: i64 = 60;

    /// An empty fleet with nothing published.
    #[must_use]
    pub fn new() -> Self {
        Self::with_lease_ttl_secs(Self::DEFAULT_LEASE_TTL_SECS)
    }

    /// An empty fleet with a custom lease TTL (floored at one second).
    #[must_use]
    pub fn with_lease_ttl_secs(lease_ttl_secs: i64) -> Self {
        Self {
            lease_ttl_secs: lease_ttl_secs.max(1),
            nodes: BTreeMap::new(),
            bundle: PolicyBundle::empty(),
        }
    }

    /// The fleet's lease TTL in seconds.
    #[must_use]
    pub fn lease_ttl_secs(&self) -> i64 {
        self.lease_ttl_secs
    }

    /// The currently published bundle.
    #[must_use]
    pub fn bundle(&self) -> &PolicyBundle {
        &self.bundle
    }

    /// Publish a new generation of policy to the fleet.
    ///
    /// Refused when the digest does not match the contents, or when the
    /// generation is not strictly newer than what is already published —
    /// the two ways a fleet could otherwise be walked backwards onto a
    /// looser policy.
    pub fn publish(&mut self, bundle: PolicyBundle) -> Result<(), EngineError> {
        if !bundle.verify() {
            return Err(EngineError::Precondition(format!(
                "policy bundle generation {} does not match its digest",
                bundle.generation
            )));
        }
        if !bundle.supersedes(&self.bundle) {
            return Err(EngineError::Precondition(format!(
                "policy bundle generation {} does not supersede the published generation {}",
                bundle.generation, self.bundle.generation
            )));
        }
        self.bundle = bundle;
        Ok(())
    }

    /// Add or replace a node and hand back the bundle it must apply.
    ///
    /// Re-registering an existing id replaces its record: a node that
    /// restarts on a new version says so by registering again, and its
    /// claim to be running a policy is cleared until it heartbeats.
    pub fn register(&mut self, node: FleetNode) -> &PolicyBundle {
        self.nodes.insert(node.id.clone(), node);
        &self.bundle
    }

    /// Renew a node's lease and record what it is running.
    ///
    /// Returns the bundle the node still has to apply, or `None` when it
    /// is already current. An unknown id is an error, not an implicit
    /// registration.
    pub fn heartbeat(
        &mut self,
        id: &str,
        now: DateTime<Utc>,
        applied_generation: u64,
        applied_digest: &str,
    ) -> Result<Option<&PolicyBundle>, EngineError> {
        let Some(node) = self.nodes.get_mut(id) else {
            return Err(EngineError::Precondition(format!(
                "node '{id}' is not registered with this fleet"
            )));
        };
        node.renew(now, applied_generation, applied_digest);
        let current = node.is_current(&self.bundle);

        Ok((!current).then_some(&self.bundle))
    }

    /// Look up one node.
    #[must_use]
    pub fn node(&self, id: &str) -> Option<&FleetNode> {
        self.nodes.get(id)
    }

    /// Every node, in id order.
    #[must_use]
    pub fn nodes(&self) -> Vec<&FleetNode> {
        self.nodes.values().collect()
    }

    /// How many nodes the control plane knows about.
    #[must_use]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the fleet is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Cordon a node off, or let it take work again.
    ///
    /// Returns the node's new state. An unknown id is an error rather
    /// than a silent no-op, for the same reason [`heartbeat`] refuses
    /// one: an operator draining a node that is not in the fleet has
    /// mistyped something, and answering "done" would let them go on to
    /// switch the wrong machine off.
    ///
    /// Note that [`register`] replaces a node's whole record, so a node
    /// that restarts and registers again arrives uncordoned. That is the
    /// honest reading — a cordon is a statement about the machine the
    /// control plane was talking to, and a restarted node is asking to
    /// join afresh — but it means an operator draining a node for
    /// maintenance should stop it, not expect the cordon to survive it.
    ///
    /// [`heartbeat`]: FleetRegistry::heartbeat
    /// [`register`]: FleetRegistry::register
    pub fn cordon(&mut self, id: &str, draining: bool) -> Result<bool, EngineError> {
        let Some(node) = self.nodes.get_mut(id) else {
            return Err(EngineError::Precondition(format!(
                "node '{id}' is not registered with this fleet"
            )));
        };
        if draining {
            node.cordon();
        } else {
            node.uncordon();
        }
        Ok(node.draining)
    }

    /// Whether `id` is cordoned off. An unknown node takes no work, so
    /// it reads as cordoned.
    #[must_use]
    pub fn is_draining(&self, id: &str) -> bool {
        self.nodes.get(id).is_none_or(|node| node.draining)
    }

    /// Drop a node the operator has retired. Returns whether it existed.
    pub fn forget(&mut self, id: &str) -> bool {
        self.nodes.remove(id).is_some()
    }

    /// Drop every node past the miss allowance, returning their ids.
    pub fn evict_expired(&mut self, now: DateTime<Utc>) -> Vec<String> {
        let ttl = self.lease_ttl_secs;
        let expired: Vec<String> = self
            .nodes
            .values()
            .filter(|node| !node.health(now, ttl).is_live())
            .map(|node| node.id.clone())
            .collect();

        for id in &expired {
            self.nodes.remove(id);
        }
        expired
    }

    /// The fleet rollup at `now`.
    #[must_use]
    pub fn status(&self, now: DateTime<Utc>) -> FleetStatus {
        let mut status = FleetStatus {
            total: self.nodes.len(),
            generation: self.bundle.generation,
            digest: self.bundle.digest.clone(),
            ..FleetStatus::default()
        };

        for node in self.nodes.values() {
            match node.health(now, self.lease_ttl_secs) {
                NodeHealth::Ready => status.ready += 1,
                NodeHealth::Stale => status.stale += 1,
                NodeHealth::Unreachable => status.unreachable += 1,
            }
            if node.is_current(&self.bundle) {
                status.on_current += 1;
            } else {
                status.drifted.push(node.id.clone());
            }
        }
        status
    }
}

impl Default for FleetRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "fleet_registry.tests.rs"]
mod tests;
