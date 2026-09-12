use std::path::PathBuf;
use std::sync::Arc;

use magistr_engine::native::NativeConfig;

use super::WorkspaceEvidence;

/// The store configuration a production writer is admitted on: the same
/// environment-derived store `NativeConfig::from_lookup` resolves, with this
/// project's [`WorkspaceEvidence`] attached. Every host admission — the
/// MCP service, the loop host, the cockpit host — goes through here, so no
/// writer opens without the capability that binds its checks to the source
/// and lets its chunks complete.
#[must_use]
pub fn writer_config(
    project_root: impl Into<PathBuf>,
    lookup: impl Fn(&str) -> Option<String>,
) -> NativeConfig {
    let root = project_root.into();
    let evidence = WorkspaceEvidence::new(root.clone());
    NativeConfig::from_lookup(root, lookup).with_evidence(Arc::new(evidence))
}
