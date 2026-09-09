use serde::{Deserialize, Serialize};

use super::{
    AgreementError, AuthorityAction, PeerIdentity, PlanBody, PlanReference, ReconciliationAction,
    WorkerModel,
};

/// Untrusted peer intent. Identity, observations and verified receipts are host inputs.
/// Use TaskAuthority::receive_peer for authorization; decoding alone grants nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PeerRequest {
    AcknowledgeBriefing {
        briefing: PlanReference,
    },
    Propose {
        base: PlanReference,
        body: Box<PlanBody>,
    },
    Reconcile {
        base: PlanReference,
        body: Box<PlanBody>,
    },
    Approve {
        plan: PlanReference,
    },
    Disagree {
        plan: PlanReference,
        reason: String,
    },
    Object {
        plan: PlanReference,
        id: String,
        tasks: Vec<String>,
        reason: String,
    },
    Resolve {
        plan: PlanReference,
        id: String,
    },
    IssueLease {
        plan: PlanReference,
        task: String,
        expires_unix_ms: u64,
    },
    AdmitWorker {
        plan: PlanReference,
        task: String,
        lease_generation: u64,
        id: String,
        worker: WorkerModel,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope<T> {
    schema_version: u16,
    request: T,
}

impl PeerRequest {
    /// Maximum encoded message size, including the envelope and whitespace.
    pub const MAX_JSON_BYTES: usize = 64 * 1024;

    /// Encode one versioned message. Validity against a run is checked on receipt.
    pub fn to_json(&self) -> Result<Vec<u8>, AgreementError> {
        let bytes = serde_json::to_vec(&Envelope {
            schema_version: 1,
            request: self,
        })?;
        Self::require_size(&bytes)?;
        Ok(bytes)
    }

    pub(super) fn decode(bytes: &[u8]) -> Result<Self, AgreementError> {
        Self::require_size(bytes)?;
        let envelope: Envelope<Self> = serde_json::from_slice(bytes)?;
        if envelope.schema_version != 1 {
            return Err(AgreementError::Invalid(
                "unsupported peer request version".into(),
            ));
        }
        Ok(envelope.request)
    }

    fn require_size(bytes: &[u8]) -> Result<(), AgreementError> {
        if bytes.len() > Self::MAX_JSON_BYTES {
            return Err(AgreementError::Invalid(
                "peer request exceeds 64 KiB".into(),
            ));
        }
        Ok(())
    }

    pub(super) fn into_action(self, peer: PeerIdentity) -> AuthorityAction {
        match self {
            Self::AcknowledgeBriefing { briefing } => {
                AuthorityAction::AcknowledgeBriefing { peer, briefing }
            }
            Self::Propose { base, body } => AuthorityAction::Protocol {
                action: ReconciliationAction::Propose { peer, base, body },
            },
            Self::Reconcile { base, body } => AuthorityAction::Protocol {
                action: ReconciliationAction::Reconcile { base, body },
            },
            Self::Approve { plan } => AuthorityAction::Protocol {
                action: ReconciliationAction::Approve { peer, plan },
            },
            Self::Disagree { plan, reason } => AuthorityAction::Protocol {
                action: ReconciliationAction::Disagree { peer, plan, reason },
            },
            Self::Object {
                plan,
                id,
                tasks,
                reason,
            } => AuthorityAction::Object {
                peer,
                plan,
                id,
                tasks,
                reason,
            },
            Self::Resolve { plan, id } => AuthorityAction::Resolve { peer, plan, id },
            Self::IssueLease {
                plan,
                task,
                expires_unix_ms,
            } => AuthorityAction::IssueLease {
                peer,
                plan,
                task,
                expires_unix_ms,
            },
            Self::AdmitWorker {
                plan,
                task,
                lease_generation,
                id,
                worker,
            } => AuthorityAction::AdmitWorker {
                peer,
                plan,
                task,
                lease_generation,
                id,
                worker,
            },
        }
    }
}
