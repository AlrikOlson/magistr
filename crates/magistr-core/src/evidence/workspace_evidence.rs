use std::path::{Path, PathBuf};

use magistr_engine::native::{EvidencePort, GateContract, SourceWitness, check_type_for};
use magistr_workspace::config::{ConfigDigest, ConfigLayer, ResolveOptions};
use magistr_workspace::snapshot::{SelectionPolicy, SourceSnapshot};

use super::SnapshotWitness;
use crate::config::inputs::ProjectInputs;
use crate::gates::resolve_gates;
use crate::types::ProjectConfig;

/// The evidence capability of one project root: source observed through
/// the workspace's snapshot capability, the applicable external
/// configuration layers (the workspace's organization policy and user
/// layer, resolved from the inputs captured at composition) observed
/// beside it, and gate contracts resolved from the project configuration
/// as it is at the moment of asking. Source and configuration observations
/// are lock-free. Receipt retention reads the loop checkpoint under the caller's
/// ledger lock, without acquiring another lock or writing any file.
#[derive(Debug, Clone)]
pub struct WorkspaceEvidence {
    root: PathBuf,
    policy: SelectionPolicy,
    layers: Vec<(ConfigLayer, PathBuf)>,
}

impl WorkspaceEvidence {
    /// The capability for the captured inputs' project root, under the
    /// default selection policy, over the external layers the inputs'
    /// workspace environment resolves.
    #[must_use]
    pub fn from_inputs(inputs: &ProjectInputs) -> Self {
        Self::new(inputs.project_root(), &inputs.workspace_resolve_options())
    }

    /// The capability for `root` under the default selection policy, over
    /// the external layers `resolve` names.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>, resolve: &ResolveOptions) -> Self {
        Self {
            root: root.into(),
            policy: SelectionPolicy::default(),
            layers: resolve.external_layer_paths().to_vec(),
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

    /// The external configuration layers every observation digests.
    #[must_use]
    pub fn layers(&self) -> &[(ConfigLayer, PathBuf)] {
        &self.layers
    }

    /// The gates `root`'s project configuration requires, as it is at the
    /// moment of asking — the one resolver behind [`EvidencePort::required_gates`],
    /// shared with a host that resolves the same set without a writer.
    ///
    /// # Errors
    /// The configuration cannot be read or resolved.
    pub fn gate_contracts(root: &Path) -> Result<Vec<GateContract>, String> {
        // A plain read: the config loader that repairs a stale stack on
        // disk would move the source revision it is being asked about.
        let path = crate::paths::project_json(root);
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

impl EvidencePort for WorkspaceEvidence {
    fn retained_receipts(&self) -> Result<std::collections::BTreeSet<String>, String> {
        use crate::workflow::iteration::IterationState;
        use magistr_domain::workflow::OperationKind;
        let path = self.root.join(".magistr/loop-state.json");
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Default::default());
            }
            Err(error) => {
                return Err(format!(
                    "Cannot read the saved loop before clearing check history: {error}"
                ));
            }
        };
        let state: IterationState = serde_json::from_slice(&bytes).map_err(|error| {
            format!("Cannot read the saved loop before clearing check history: {error}")
        })?;
        Ok(state
            .operations
            .records()
            .iter()
            .filter(|record| record.kind == OperationKind::Receipt && !record.outcome.is_settled())
            .filter_map(|record| {
                record
                    .external
                    .as_deref()?
                    .strip_prefix("receipt:")
                    .map(str::to_owned)
            })
            .collect())
    }

    fn observe(&self) -> Result<Box<dyn SourceWitness>, String> {
        let snapshot = SourceSnapshot::capture(&self.root, self.policy.clone())
            .map_err(|error| error.to_string())?;
        let configuration =
            ConfigDigest::observe(&self.layers).map_err(|error| error.to_string())?;
        Ok(Box::new(SnapshotWitness::new(
            self.root.clone(),
            snapshot,
            self.layers.clone(),
            configuration,
        )))
    }

    fn required_gates(&self) -> Result<Vec<GateContract>, String> {
        Self::gate_contracts(&self.root)
    }
}

#[cfg(test)]
#[path = "workspace_evidence.tests.rs"]
mod tests;
