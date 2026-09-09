use super::{InstructionLease, WorkerModel};

/// One admitted worker start, bound to its supervisor's instruction fence.
/// Not serializable; a checkpoint retains history, never renewable dispatch authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerPermit {
    pub(super) id: String,
    pub(super) lease: InstructionLease,
    pub(super) worker: WorkerModel,
    pub(super) attempt: u32,
}

impl WorkerPermit {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn lease(&self) -> &InstructionLease {
        &self.lease
    }

    #[must_use]
    pub fn worker(&self) -> &WorkerModel {
        &self.worker
    }

    /// One-based attempt for this stable task ID, retained across revisions.
    #[must_use]
    pub fn attempt(&self) -> u32 {
        self.attempt
    }
}
