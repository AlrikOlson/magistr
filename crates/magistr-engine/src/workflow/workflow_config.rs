use serde::{Deserialize, Serialize};

use super::lifecycle_hook::LifecycleHook;
use super::quality_gate_config::QualityGateConfig;

/// Configuration for a workflow iteration.
///
/// This is a value object — it carries the settings needed to orchestrate
/// a plan-execute-verify cycle. File paths are strings here (not `PathBuf`)
/// because the engine layer doesn't do IO.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowConfig {
    /// Project directory (as string — no IO in engine layer).
    pub project_dir: String,
    /// Spec file path relative to project dir.
    pub spec_path: String,
    /// Quality gate commands (check, test, lint).
    pub quality_gate: Option<QualityGateConfig>,
    /// Maximum iterations for loop mode.
    pub max_iterations: u32,
    /// Lifecycle hooks triggered at workflow phase transitions and events.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lifecycle_hooks: Vec<LifecycleHook>,
}
