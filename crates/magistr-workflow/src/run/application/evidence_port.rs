use super::{CheckContract, CheckRequest, SourceWitness};

/// The host capability the workflow consumes to bind checks to the inputs
/// they certify and to know which checks a completion must present. The
/// workflow never reads a tree or a check configuration itself: the host
/// composes this port once from its workspace's source-snapshot capability
/// and its check resolver, and every server-executed check — the host's own
/// check runner, its loop, and the native `check_step` — goes through it.
///
/// Source observations and check resolution run outside the engine lock.
/// Retention reads run inside the check_result lock and must never call back into
/// the workflow or wait for a check_result writer.
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

    /// Compare certified inputs through the capability that retained their proof.
    /// Providers without path-level history retain exact observation matching.
    fn inputs_match(
        &self,
        before: &super::SourceObservation,
        now: &super::SourceObservation,
    ) -> Result<bool, String> {
        Ok(before.drift_to(now).is_none())
    }

    /// Only the host can declare a command suitable for check_result reuse.
    fn can_reuse(&self, _request: &CheckRequest) -> bool {
        false
    }

    /// One deadline contract across native and transport callers.
    fn check_budget(
        &self,
        _name: &str,
        _command: Option<&str>,
        per_call: Option<u64>,
    ) -> Result<super::CheckBudget, String> {
        super::CheckBudget::resolve(per_call, None, None)
    }

    /// Resource admission shared with every other check runner.
    fn check_schedule(
        &self,
        _name: &str,
        _command: Option<&str>,
    ) -> Result<magistr_process::CheckSchedule, String> {
        Ok(Default::default())
    }

    /// The checks the project currently requires, from its configuration as
    /// it is at the moment of asking.
    ///
    /// # Errors
    /// The configuration cannot be read or resolved.
    fn required_checks(&self) -> Result<Vec<CheckContract>, String>;

    /// Exact check_result IDs still referenced by unresolved host operations.
    /// Called under the ledger lock, after the current run is pinned. Hosts
    /// must persist references before settling check_results, and must override
    /// this method when they keep an external recovery checkpoint.
    /// An unreadable checkpoint must return an error, never an empty set.
    fn retained_check_results(&self) -> Result<std::collections::BTreeSet<String>, String> {
        Ok(std::collections::BTreeSet::new())
    }
}
