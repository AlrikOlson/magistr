use super::{PeerIdentity, PlanReference};

/// Opaque, revocable owner instruction authority. Validate at each operation boundary.
/// Not serializable: restoration requires a newly issued lease.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstructionLease {
    pub(super) plan: PlanReference,
    pub(super) task: String,
    pub(super) owner: PeerIdentity,
    pub(super) generation: u64,
    pub(super) expires_unix_ms: u64,
}

impl InstructionLease {
    #[must_use]
    pub fn plan(&self) -> &PlanReference {
        &self.plan
    }

    #[must_use]
    pub fn task(&self) -> &str {
        &self.task
    }

    #[must_use]
    pub fn owner(&self) -> &PeerIdentity {
        &self.owner
    }

    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }

    #[must_use]
    pub fn expires_unix_ms(&self) -> u64 {
        self.expires_unix_ms
    }
}
