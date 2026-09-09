use serde::{Deserialize, Serialize};

use super::{PeerIdentity, PlanReference, ReconciliationAction, WorkerModel};

/// Host-attributed authority input. Peer identities must never come from model payloads.
/// Route untrusted peer bytes through TaskAuthority::receive_peer instead of decoding this type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AuthorityAction {
    /// Host-only confirmation that these exact bytes were delivered to the fixed session.
    DeliverBriefing {
        peer: PeerIdentity,
        briefing: PlanReference,
    },
    /// Peer acknowledgement of delivery; never a plan approval.
    AcknowledgeBriefing {
        peer: PeerIdentity,
        briefing: PlanReference,
    },
    /// A new start for an existing task; the host supplies authenticated attribution.
    AdmitWorker {
        peer: PeerIdentity,
        plan: PlanReference,
        task: String,
        lease_generation: u64,
        id: String,
        worker: WorkerModel,
    },
    /// The host has verified completion and termination, not merely a model's claim.
    AcceptWorker {
        peer: PeerIdentity,
        id: String,
        receipt: String,
    },
    /// Host-only termination evidence; releases an unsettled attempt without acceptance.
    RetireWorker {
        id: String,
        receipt: String,
    },
    Protocol {
        action: ReconciliationAction,
    },
    Object {
        peer: PeerIdentity,
        plan: PlanReference,
        id: String,
        tasks: Vec<String>,
        reason: String,
    },
    Resolve {
        peer: PeerIdentity,
        plan: PlanReference,
        id: String,
    },
    IssueLease {
        peer: PeerIdentity,
        plan: PlanReference,
        task: String,
        expires_unix_ms: u64,
    },
}

impl AuthorityAction {
    pub(super) fn observe() -> Self {
        Self::Protocol {
            action: ReconciliationAction::Observe,
        }
    }
}
