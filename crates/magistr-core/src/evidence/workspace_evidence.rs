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

    fn check_policy(
        &self,
        name: &str,
        command: Option<&str>,
    ) -> Result<
        (
            crate::types::VerificationConfig,
            Option<crate::types::GatePolicy>,
        ),
        String,
    > {
        let path = crate::paths::project_json(&self.root).map_err(|error| error.to_string())?;
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok((Default::default(), None));
            }
            Err(error) => return Err(error.to_string()),
        };
        let config: ProjectConfig =
            serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
        let matched = resolve_gates(&config, name).ok().and_then(|specs| {
            specs
                .into_iter()
                .find(|spec| Some(spec.command.trim()) == command.map(str::trim))
                .map(|spec| spec.name)
        });
        let policy = matched.and_then(|name| config.verification.gates.get(&name).cloned());
        Ok((config.verification, policy))
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
        let path = crate::paths::project_json(root).map_err(|error| error.to_string())?;
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
    fn for_working_copy(&self, root: &Path) -> Result<std::sync::Arc<dyn EvidencePort>, String> {
        let mut evidence = self.clone();
        evidence.root = root.canonicalize().map_err(|error| error.to_string())?;
        Ok(std::sync::Arc::new(evidence))
    }

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
        self.observe_check("", None, &[])
    }

    fn observe_check(
        &self,
        name: &str,
        command: Option<&str>,
        inputs: &[String],
    ) -> Result<Box<dyn SourceWitness>, String> {
        if !self.root.exists() {
            return Err(format!(
                "The workspace {} does not exist.",
                self.root.display()
            ));
        }
        let (_, policy) = self.check_policy(name, command)?;
        let declared = policy.as_ref().map(|policy| &policy.inputs);
        let mut sources = inputs.to_vec();
        if let Some(declared) = declared {
            if declared.is_empty() {
                sources.clear();
            } else {
                sources.extend(declared.iter().cloned());
            }
        }
        let policy = self.policy.clone().with_sources(sources);
        let snapshot =
            SourceSnapshot::capture(&self.root, policy).map_err(|error| error.to_string())?;
        let mut layers = self.layers.clone();
        layers.push((
            ConfigLayer::Workspace,
            crate::paths::project_json(&self.root).map_err(|error| error.to_string())?,
        ));
        let configuration = ConfigDigest::observe(&layers).map_err(|error| error.to_string())?;
        Ok(Box::new(
            SnapshotWitness::new(self.root.clone(), snapshot, layers, configuration)
                .with_execution(command)?,
        ))
    }

    fn can_reuse(&self, request: &magistr_engine::native::CheckRequest) -> bool {
        let Ok((_, policy)) = self.check_policy(&request.name, request.command.as_deref()) else {
            return false;
        };
        if let Some(reuse) = policy.and_then(|policy| policy.reuse) {
            return reuse;
        }
        request.command.as_deref().is_some_and(reusable_command)
    }

    fn check_timeout_secs(&self, name: &str, command: Option<&str>) -> Result<u64, String> {
        let (config, policy) = self.check_policy(name, command)?;
        let timeout = policy
            .and_then(|policy| policy.timeout_secs)
            .unwrap_or(config.timeout_secs);
        if !(1..=3600).contains(&timeout) {
            return Err("Check timeout must be between 1 and 3600 seconds.".into());
        }
        Ok(timeout)
    }

    fn required_gates(&self) -> Result<Vec<GateContract>, String> {
        Self::gate_contracts(&self.root)
    }
}

#[cfg(test)]
#[path = "workspace_evidence.tests.rs"]
mod tests;

/// Only recognizable local verification commands are reusable by default.
/// Opaque scripts and network checks need an explicit configured declaration.
fn reusable_command(command: &str) -> bool {
    if command.trim().is_empty() {
        return false;
    }
    command.split("&&").all(|part| {
        let part = part.trim();
        if part
            .chars()
            .any(|ch| matches!(ch, ';' | '|' | '&' | '\n' | '`' | '$' | '>' | '<'))
        {
            return false;
        }
        let words: Vec<_> = part.split_whitespace().collect();
        matches!(
            words.as_slice(),
            ["cargo", "check" | "clippy", ..]
                | ["cargo", "test", ..]
                | ["cargo", "nextest", "run", ..]
        ) || (words.starts_with(&["cargo", "fmt"]) && words.contains(&"--check"))
    })
}
