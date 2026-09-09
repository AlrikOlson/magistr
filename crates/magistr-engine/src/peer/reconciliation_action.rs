use serde::{Deserialize, Serialize};

use super::{PeerIdentity, PlanBody, PlanReference};

/// An input from the trusted host. Peer identities MUST come from authenticated
/// session context, never the model payload. Reconcile carries no voting authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReconciliationAction {
    Observe,
    Propose {
        peer: PeerIdentity,
        base: PlanReference,
        body: Box<PlanBody>,
    },
    Reconcile {
        base: PlanReference,
        body: Box<PlanBody>,
    },
    Approve {
        peer: PeerIdentity,
        plan: PlanReference,
    },
    Disagree {
        peer: PeerIdentity,
        plan: PlanReference,
        reason: String,
    },
    PeerUnavailable {
        peer: PeerIdentity,
    },
}
