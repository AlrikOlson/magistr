use std::path::PathBuf;
use std::sync::Arc;

use magistr_engine::native::NativeConfig;

/// Capture production configuration, including the platform temporary-directory
/// fallback, and attach the evidence capability required by every writer.
#[must_use]
pub fn writer_config_from_env(project_root: impl Into<PathBuf>) -> NativeConfig {
    let root = project_root.into();
    let evidence = super::WorkspaceEvidence::new(root.clone());
    NativeConfig::from_env(root).with_evidence(Arc::new(evidence))
}

#[cfg(test)]
#[path = "writer_config_from_env.tests.rs"]
mod tests;
