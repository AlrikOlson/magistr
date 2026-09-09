use serde::{Deserialize, Serialize};

use super::{AgreementError, PeerEvidence, PlanBody, PlanReference, PlanRevision, RunContract};

/// Immutable common evidence and initial plan, with no proposal or approval state.
#[derive(Debug)]
pub struct PeerBriefing {
    revision: PlanRevision,
    evidence: PeerEvidence,
    bytes: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Packet {
    schema_version: u16,
    plan: PlanReference,
    contract: RunContract,
    body: PlanBody,
    evidence: PeerEvidence,
}

impl PeerBriefing {
    pub const MAX_JSON_BYTES: usize = 256 * 1024;

    /// The caller deliberately sets body.context using evidence.context(constraints).
    /// Evidence is never silently substituted beneath an existing plan reference.
    pub fn new(
        contract: RunContract,
        body: PlanBody,
        evidence: PeerEvidence,
    ) -> Result<Self, AgreementError> {
        for (name, text) in [
            ("roadmap evidence", &evidence.roadmap),
            ("code evidence", &evidence.code),
            ("routing history", &evidence.routing_history),
        ] {
            super::require_text(name, text)?;
        }
        if body.context != evidence.context(body.context.constraints.clone()) {
            return Err(AgreementError::Invalid(
                "briefing evidence does not match plan context".into(),
            ));
        }
        let revision = PlanRevision::seal(contract, body, 1)?;
        let bytes = serde_json::to_vec(&Packet {
            schema_version: 1,
            plan: revision.reference(),
            contract: revision.contract().clone(),
            body: revision.body().clone(),
            evidence: evidence.clone(),
        })?;
        require_size(&bytes)?;
        Ok(Self {
            revision,
            evidence,
            bytes,
        })
    }

    #[must_use]
    pub fn revision(&self) -> &PlanRevision {
        &self.revision
    }

    #[must_use]
    pub fn evidence(&self) -> &PeerEvidence {
        &self.evidence
    }

    /// Identical canonical UTF-8 JSON for each fixed peer; contains no private proposals.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn to_json(&self) -> Result<Vec<u8>, AgreementError> {
        Ok(self.bytes.clone())
    }

    pub fn from_json(
        bytes: &[u8],
        expected_contract: &RunContract,
    ) -> Result<Self, AgreementError> {
        require_size(bytes)?;
        let packet: Packet = serde_json::from_slice(bytes)?;
        if packet.schema_version != 1 {
            return Err(AgreementError::Invalid(
                "unsupported peer briefing version".into(),
            ));
        }
        if &packet.contract != expected_contract {
            return Err(AgreementError::ContractMismatch);
        }
        let value = Self::new(packet.contract, packet.body, packet.evidence)?;
        if value.revision.reference() != packet.plan {
            return Err(AgreementError::DigestMismatch);
        }
        Ok(value)
    }
}

fn require_size(bytes: &[u8]) -> Result<(), AgreementError> {
    if bytes.len() > PeerBriefing::MAX_JSON_BYTES {
        return Err(AgreementError::Invalid(
            "peer briefing exceeds 256 KiB".into(),
        ));
    }
    Ok(())
}
