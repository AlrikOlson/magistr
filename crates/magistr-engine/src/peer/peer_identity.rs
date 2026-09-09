use serde::{Deserialize, Serialize};

use super::{Peer, WorkerModel};

/// A session binding supplied by the trusted runtime host, never by a model response.
/// This value carries identity; constructing it does not authenticate a runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PeerIdentity {
    pub peer: Peer,
    pub session_id: String,
    pub model: WorkerModel,
}
