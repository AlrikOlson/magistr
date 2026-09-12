use std::path::PathBuf;
use std::sync::Arc;

use magistr_engine::native::NativeConfig;

use super::WorkspaceEvidence;

/// Explicit-input native configuration with this project's evidence attached.
/// Tests and injected hosts use this environment-independent resolver. Production
/// entry points use [`super::writer_config_from_env`] to retain the operating
/// system's temporary-directory fallback as part of their captured host inputs.
#[must_use]
pub fn writer_config(
    project_root: impl Into<PathBuf>,
    lookup: impl Fn(&str) -> Option<String>,
) -> NativeConfig {
    let root = project_root.into();
    let evidence = WorkspaceEvidence::new(root.clone());
    NativeConfig::from_lookup(root, lookup).with_evidence(Arc::new(evidence))
}
