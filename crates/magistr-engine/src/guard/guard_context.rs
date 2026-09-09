use super::ship_guard_state::ShipGuardState;

/// Context passed to guard hooks so they can inspect project state.
///
/// Uses `String` paths (not `PathBuf`) because the engine layer is IO-free.
/// Infrastructure adapters convert `PathBuf` to `String` at the boundary,
/// and read the ship engine's status into [`ShipGuardState`] before handing
/// it here — the guards never open a store or a plan file themselves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardContext {
    /// The project root directory.
    pub project_dir: String,
    /// Path to the spec file (e.g., `ROADMAP.md`).
    pub spec_path: String,
    /// The execution-trace slice the plan guard keys on (objective, tasks,
    /// recorded checks) — what `ship_status` answered when the context was
    /// built.
    pub ship: ShipGuardState,
}

impl GuardContext {
    /// Create a new guard context.
    pub fn new(
        project_dir: impl Into<String>,
        spec_path: impl Into<String>,
        ship: ShipGuardState,
    ) -> Self {
        Self {
            project_dir: project_dir.into(),
            spec_path: spec_path.into(),
            ship,
        }
    }
}
