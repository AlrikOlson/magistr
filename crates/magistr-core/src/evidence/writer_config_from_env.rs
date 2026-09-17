use std::path::PathBuf;
use std::sync::Arc;

use magistr_engine::native::NativeConfig;

use crate::config::inputs::ProjectInputs;

/// Capture production configuration, including the platform temporary-directory
/// fallback, and attach the evidence capability required by every writer —
/// over the external configuration layers the process environment names.
#[must_use]
pub fn writer_config_from_env(project_root: impl Into<PathBuf>) -> NativeConfig {
    let root = project_root.into();
    let inputs = ProjectInputs::from_env(root.clone());
    let evidence = super::WorkspaceEvidence::from_inputs(&inputs);
    NativeConfig::from_env(root).with_evidence(Arc::new(evidence))
}

#[cfg(test)]
#[path = "writer_config_from_env.tests.rs"]
mod tests;
