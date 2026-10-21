use super::{CheckRequest, GateContract, SourceWitness};

/// The host capability the workflow consumes to bind checks to the inputs
/// they certify and to know which gates a completion must present. The
/// workflow never reads a tree or a gate configuration itself: the host
/// composes this port once from its workspace's source-snapshot capability
/// and its gate resolver, and every server-executed check — the host's own
/// gate runner, its loop, and the native `ship_check` — goes through it.
///
/// Source observations and gate resolution run outside the engine lock.
/// Retention reads run inside the receipt lock and must never call back into
/// the workflow or wait for a receipt writer.
pub trait EvidencePort: Send + Sync {
    /// Rebind captured host policy after the project owner validates repository membership.
    fn for_working_copy(
        &self,
        _root: &std::path::Path,
    ) -> Result<std::sync::Arc<dyn EvidencePort>, String> {
        Err("This evidence provider cannot open another working copy.".into())
    }

    /// Observe the workspace now, lock-free.
    ///
    /// # Errors
    /// The workspace cannot be observed (missing root, refused selection).
    fn observe(&self) -> Result<Box<dyn SourceWitness>, String>;

    /// Observe the declared inputs of this check. Providers that cannot
    /// narrow a selection retain their conservative whole-workspace behavior.
    fn observe_check(
        &self,
        _name: &str,
        _command: Option<&str>,
        _inputs: &[String],
    ) -> Result<Box<dyn SourceWitness>, String> {
        self.observe()
    }

    /// Only the host can declare a command suitable for receipt reuse.
    fn can_reuse(&self, _request: &CheckRequest) -> bool {
        false
    }

    /// One deadline contract across native and transport callers.
    fn check_timeout_secs(&self, _name: &str, _command: Option<&str>) -> Result<u64, String> {
        Ok(super::CheckOptions::DEFAULT_TIMEOUT_SECS)
    }

    /// Resource admission shared with every other check runner.
    fn check_schedule(
        &self,
        _name: &str,
        _command: Option<&str>,
    ) -> Result<magistr_process::GateSchedule, String> {
        Ok(Default::default())
    }

    /// The gates the project currently requires, from its configuration as
    /// it is at the moment of asking.
    ///
    /// # Errors
    /// The configuration cannot be read or resolved.
    fn required_gates(&self) -> Result<Vec<GateContract>, String>;

    /// Exact receipt IDs still referenced by unresolved host operations.
    /// Called under the ledger lock, after the current run is pinned. Hosts
    /// must persist references before settling receipts, and must override
    /// this method when they keep an external recovery checkpoint.
    /// An unreadable checkpoint must return an error, never an empty set.
    fn retained_receipts(&self) -> Result<std::collections::BTreeSet<String>, String> {
        Ok(std::collections::BTreeSet::new())
    }
}
