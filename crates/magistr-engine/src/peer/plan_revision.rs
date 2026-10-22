use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{AgreementError, PlanBody, PlanReference, RunContract};

const SCHEMA_VERSION: u32 = 1;
const HASH_DOMAIN: &[u8] = b"magistr-peer-plan-v1\0";

/// A validated, immutable revision. It cannot be deserialized without validation.
/// The digest detects changes; it is not a signature or proof of peer authentication.
#[derive(Debug)]
pub struct PlanRevision {
    snapshot: Snapshot,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    schema_version: u32,
    revision: u64,
    contract: RunContract,
    body: PlanBody,
    digest: String,
}

impl PlanRevision {
    pub(super) fn seal(
        contract: RunContract,
        body: PlanBody,
        revision: u64,
    ) -> Result<Self, AgreementError> {
        contract.validate()?;
        body.validate(&contract)?;
        if revision == 0 {
            return Err(AgreementError::Invalid("revision must be positive".into()));
        }
        let mut snapshot = Snapshot {
            schema_version: SCHEMA_VERSION,
            revision,
            contract,
            body,
            digest: String::new(),
        };
        snapshot.digest = snapshot.compute_digest()?;
        Ok(Self { snapshot })
    }

    #[must_use]
    pub fn contract(&self) -> &RunContract {
        &self.snapshot.contract
    }

    #[must_use]
    pub fn body(&self) -> &PlanBody {
        &self.snapshot.body
    }

    #[must_use]
    pub fn reference(&self) -> PlanReference {
        PlanReference {
            project_id: self.snapshot.contract.project_id.clone(),
            run_id: self.snapshot.contract.run_id.clone(),
            revision: self.snapshot.revision,
            digest: self.snapshot.digest.clone(),
        }
    }

    /// Save revision data only. No votes or dispatch authority are serialized.
    pub fn to_json(&self) -> Result<Vec<u8>, AgreementError> {
        Ok(serde_json::to_vec(&self.snapshot)?)
    }

    /// Restore against the host's trusted original contract, validating data and hash.
    /// The host must also enforce latest-revision/replay rules in its durable store.
    pub fn from_json(
        bytes: &[u8],
        expected_contract: &RunContract,
    ) -> Result<Self, AgreementError> {
        let snapshot: Snapshot = serde_json::from_slice(bytes)?;
        if snapshot.schema_version != SCHEMA_VERSION {
            return Err(AgreementError::Invalid(
                "unsupported peer plan schema version".into(),
            ));
        }
        if &snapshot.contract != expected_contract {
            return Err(AgreementError::ContractMismatch);
        }
        let restored = Self::seal(snapshot.contract, snapshot.body, snapshot.revision)?;
        if restored.snapshot.digest != snapshot.digest {
            return Err(AgreementError::DigestMismatch);
        }
        Ok(restored)
    }

    pub(super) fn next(&self, body: PlanBody) -> Result<Self, AgreementError> {
        let revision = self
            .snapshot
            .revision
            .checked_add(1)
            .ok_or(AgreementError::RevisionOverflow)?;
        Self::seal(self.snapshot.contract.clone(), body, revision)
    }
}

impl Snapshot {
    fn compute_digest(&self) -> Result<String, AgreementError> {
        // v1 encoding: compact serde JSON tuple [schema, revision, contract, body].
        // All nested objects are typed structs in declaration order; no maps/floats.
        let bytes = serde_json::to_vec(&(
            self.schema_version,
            self.revision,
            &self.contract,
            &self.body,
        ))?;
        let mut hash = Sha256::new();
        hash.update(HASH_DOMAIN);
        hash.update(bytes);
        Ok(hex::encode(hash.finalize()))
    }
}
