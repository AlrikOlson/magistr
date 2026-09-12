use super::{GateContract, SourceWitness};

/// The host capability the workflow consumes to bind checks to the inputs
/// they certify and to know which gates a completion must present. The
/// workflow never reads a tree or a gate configuration itself: the host
/// composes this port once from its workspace's source-snapshot capability
/// and its gate resolver, and every server-executed check — the host's own
/// gate runner, its loop, and the native `ship_check` — goes through it.
///
/// Every call runs outside the workflow's engine lock; an observation may
/// hash a whole tree and must never wait on a mutation.
pub trait EvidencePort: Send + Sync {
    /// Observe the workspace now, lock-free.
    ///
    /// # Errors
    /// The workspace cannot be observed (missing root, refused selection).
    fn observe(&self) -> Result<Box<dyn SourceWitness>, String>;

    /// The gates the project currently requires, from its configuration as
    /// it is at the moment of asking.
    ///
    /// # Errors
    /// The configuration cannot be read or resolved.
    fn required_gates(&self) -> Result<Vec<GateContract>, String>;
}
