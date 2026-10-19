use std::path::{Component, Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{SnapshotError, workspace_identity::frame};

/// What a snapshot selects, stated so the same selection can be re-run at
/// validation. The tree walk is the workspace's own ignore-driven selection
/// (hidden files included, ignore files honored with or without a
/// repository, `.git` and `.magistr/workspace` never entered, symlinks not
/// followed but recorded). Explicit inputs are hashed even when ignored —
/// gate configuration that ordinary selection would skip, or a built
/// artifact a consumer certifies alongside the source.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SelectionPolicy {
    pub version: u32,
    /// The hard ceiling on manifest entries; exceeding it refuses the
    /// snapshot instead of truncating it.
    pub max_entries: usize,
    /// Configuration read even when ignored; a missing one is recorded as missing.
    pub config_inputs: Vec<String>,
    /// Caller-declared artifacts read even when ignored.
    pub artifact_inputs: Vec<String>,
}

impl Default for SelectionPolicy {
    fn default() -> Self {
        Self {
            version: 1,
            max_entries: 200_000,
            config_inputs: vec![
                ".magistr/project.json".into(),
                ".magistr/workspace.toml".into(),
            ],
            artifact_inputs: Vec::new(),
        }
    }
}

impl SelectionPolicy {
    /// Add one explicit artifact input (workspace-relative).
    #[must_use]
    pub fn with_artifact(mut self, path: impl Into<String>) -> Self {
        self.artifact_inputs.push(path.into());
        self
    }

    /// SHA-256 over the version, ceiling and every explicit input, framed.
    #[must_use]
    pub fn digest(&self) -> String {
        let mut digest = Sha256::new();
        frame(&mut digest, &self.version.to_le_bytes());
        frame(&mut digest, &(self.max_entries as u64).to_le_bytes());
        for input in self.explicit_inputs() {
            frame(&mut digest, input.as_bytes());
        }
        format!("{:x}", digest.finalize())
    }

    /// Configuration then artifacts, in declaration order.
    pub fn explicit_inputs(&self) -> impl Iterator<Item = &str> {
        self.config_inputs
            .iter()
            .chain(self.artifact_inputs.iter())
            .map(String::as_str)
    }

    /// Refuse an explicit input that could escape the root or name owner
    /// state: only relative, normal components, never `.git` or `.magistr/workspace`.
    pub(super) fn validate(&self) -> Result<(), SnapshotError> {
        for input in self.explicit_inputs() {
            let refuse = |reason: &str| SnapshotError::InvalidInput {
                path: input.to_owned(),
                reason: reason.to_owned(),
            };
            if crate::paths::is_owner_path(Path::new(input)) {
                return Err(refuse("owner and repository state is never a source input"));
            }
            if input.is_empty() {
                return Err(refuse("empty path"));
            }
            for component in Path::new(input).components() {
                match component {
                    Component::Normal(_) => {}
                    _ => return Err(refuse("only relative paths with normal components")),
                }
            }
        }
        Ok(())
    }
}
