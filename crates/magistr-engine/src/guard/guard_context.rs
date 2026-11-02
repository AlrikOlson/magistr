use super::run_guard_state::RunGuardState;

/// Context passed to guard hooks so they can inspect project state.
///
/// Uses `String` paths (not `PathBuf`) because the engine layer is IO-free.
/// Infrastructure adapters convert `PathBuf` to `String` at the boundary,
/// and read the run engine's records into [`RunGuardState`] before handing
/// it here — the guards never open a store or a plan file themselves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardContext {
    /// The project root directory.
    pub project_dir: String,
    /// Path to the spec file (e.g., `PLAN.md`).
    pub spec_path: String,
    /// The execution-trace slice the plan guard keys on (goal, tasks,
    /// recorded checks) — what `run_status` answered when the context was
    /// built.
    pub run: RunGuardState,
}

impl GuardContext {
    /// Create a new guard context.
    pub fn new(
        project_dir: impl Into<String>,
        spec_path: impl Into<String>,
        run: RunGuardState,
    ) -> Self {
        Self {
            project_dir: project_dir.into(),
            spec_path: spec_path.into(),
            run,
        }
    }
}
