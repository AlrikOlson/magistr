use std::path::{Path, PathBuf};

use magistr_engine::native::{EvidencePort, GateContract, SourceWitness, check_type_for};
use magistr_workspace::snapshot::{SelectionPolicy, SourceSnapshot};

use super::SnapshotWitness;
use crate::gates::resolve_gates;
use crate::types::ProjectConfig;

/// The evidence capability of one project root: source observed through
/// the workspace's snapshot capability, gate contracts resolved from the
/// project configuration as it is at the moment of asking. Both read the
/// tree without any lock and without touching the configuration file.
#[derive(Debug, Clone)]
pub struct WorkspaceEvidence {
    root: PathBuf,
    policy: SelectionPolicy,
}

impl WorkspaceEvidence {
    /// The capability for `root` under the default selection policy.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            policy: SelectionPolicy::default(),
        }
    }

    /// The project root observed.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The selection policy every observation runs under.
    #[must_use]
    pub fn policy(&self) -> &SelectionPolicy {
        &self.policy
    }
}

impl EvidencePort for WorkspaceEvidence {
    fn observe(&self) -> Result<Box<dyn SourceWitness>, String> {
        let snapshot = SourceSnapshot::capture(&self.root, self.policy.clone())
            .map_err(|error| error.to_string())?;
        Ok(Box::new(SnapshotWitness::new(self.root.clone(), snapshot)))
    }

    fn required_gates(&self) -> Result<Vec<GateContract>, String> {
        // A plain read: the config loader that repairs a stale stack on
        // disk would move the source revision it is being asked about.
        let path = crate::paths::project_json(&self.root);
        let bytes = std::fs::read(&path)
            .map_err(|error| format!("read gate configuration {}: {error}", path.display()))?;
        let config: ProjectConfig = serde_json::from_slice(&bytes)
            .map_err(|error| format!("parse gate configuration {}: {error}", path.display()))?;
        Ok(resolve_gates(&config, "all")?
            .into_iter()
            .filter(|spec| spec.required)
            .map(|spec| GateContract {
                check_type: check_type_for(&spec.name),
                name: spec.name,
                command: spec.command,
            })
            .collect())
    }
}

#[cfg(test)]
#[path = "workspace_evidence.tests.rs"]
mod tests;
