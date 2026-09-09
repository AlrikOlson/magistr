use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::{AgreementError, OutagePolicy, Peer, PeerIdentity, RunLimits, WorkerModel};

/// User-authorized contract, frozen when a plan is sealed. Changes require a new run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunContract {
    pub project_id: String,
    pub run_id: String,
    /// Fixed order: Astra then Fable. Session identity is provider-scoped.
    pub peers: [PeerIdentity; 2],
    pub allowed_worker_models: Vec<WorkerModel>,
    pub limits: RunLimits,
    pub outage_policy: OutagePolicy,
}

impl RunContract {
    pub(super) fn validate(&self) -> Result<(), AgreementError> {
        super::require_text("project_id", &self.project_id)?;
        super::require_text("run_id", &self.run_id)?;
        if self.peers[0].peer != Peer::Astra || self.peers[1].peer != Peer::Fable {
            return Err(AgreementError::Invalid(
                "peers must be Astra then Fable".into(),
            ));
        }
        for identity in &self.peers {
            super::require_text("session_id", &identity.session_id)?;
            identity.model.validate()?;
        }
        if self.peers[0].model.provider == self.peers[1].model.provider
            && self.peers[0].session_id == self.peers[1].session_id
        {
            return Err(AgreementError::Invalid(
                "voters must be distinct sessions".into(),
            ));
        }
        if self.allowed_worker_models.is_empty()
            || self
                .allowed_worker_models
                .iter()
                .collect::<BTreeSet<_>>()
                .len()
                != self.allowed_worker_models.len()
        {
            return Err(AgreementError::Invalid(
                "worker models must be nonempty and unique".into(),
            ));
        }
        for model in &self.allowed_worker_models {
            model.validate()?;
        }
        if self.limits.deadline_unix_ms == 0
            || self.limits.max_total_tokens == 0
            || self.limits.max_attempts == 0
            || self.limits.max_reconciliation_rounds == 0
        {
            return Err(AgreementError::Invalid(
                "deadline, tokens, attempts and reconciliation rounds must be positive".into(),
            ));
        }
        Ok(())
    }
}
