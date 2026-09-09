use super::{InstructionLease, ReconciliationStop, WorkerPermit};

/// A reached run limit is a recorded stop, never an instruction lease.
#[derive(Debug)]
pub enum AuthorityOutcome {
    Updated,
    Worker(Box<WorkerPermit>),
    /// Duplicate request: no new start is authorized.
    AlreadyAdmitted,
    Lease(Box<InstructionLease>),
    Stopped(ReconciliationStop),
}
