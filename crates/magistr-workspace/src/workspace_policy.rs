use std::path::{Path, PathBuf};

use crate::config::{CommandAdmission, ResolveOptions, ResolvedConfig};
use crate::contracts::{ErrorKind, WorkspaceError};
use crate::jail::JailPolicy;

/// Read-only workspace admission composed from the existing configuration and
/// path policies. User approval cannot override an invariant path refusal.
pub struct WorkspacePolicy {
    config: ResolvedConfig,
    paths: JailPolicy,
}

impl WorkspacePolicy {
    pub fn resolve(options: &ResolveOptions) -> Result<Self, WorkspaceError> {
        let paths = JailPolicy::new(&options.root)?;
        let mut options = options.clone();
        options.root = paths.root().to_path_buf();
        let config = ResolvedConfig::resolve(&options)?;
        // File-layer overrides may be relative to the host's current directory,
        // exactly as the config reader interprets them. Protect that same file.
        let layers = options
            .layer_paths()
            .into_iter()
            .map(|(_, path)| {
                std::path::absolute(path).map_err(|error| {
                    WorkspaceError::new(ErrorKind::ConfigPolicyDenied, error.to_string())
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let protected = config.effective["security.protected_paths"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .map(protected_subtree)
            .collect::<Result<Vec<_>, _>>()?;
        let paths = paths
            .with_protected_paths(layers)?
            .with_protected_paths(protected)?;
        Ok(Self { config, paths })
    }

    #[must_use]
    pub fn config(&self) -> &ResolvedConfig {
        &self.config
    }

    pub fn check_read(&self, path: impl AsRef<Path>) -> Result<PathBuf, WorkspaceError> {
        self.paths.check_read(path)
    }

    pub fn check_write(&self, path: impl AsRef<Path>) -> Result<PathBuf, WorkspaceError> {
        self.paths.check_write(path)
    }

    pub fn admit_command(
        &self,
        command: &str,
        args: &[String],
    ) -> Result<Vec<String>, WorkspaceError> {
        CommandAdmission::admit(&self.config, command, args)
    }
}

// The retained configuration examples use recursive subtree suffixes. Accept
// those explicitly; never let an unsupported glob silently protect nothing.
fn protected_subtree(value: &str) -> Result<PathBuf, WorkspaceError> {
    let path = if value == "**" {
        ""
    } else {
        value
            .strip_suffix("/**")
            .or_else(|| value.strip_suffix("\\**"))
            .unwrap_or(value)
    };
    if path.contains(['*', '?', '[', ']', '{', '}']) {
        return Err(WorkspaceError::new(
            ErrorKind::ConfigPolicyDenied,
            format!(
                "security.protected_paths supports literal paths and trailing /** subtrees; unsupported pattern {value:?}"
            ),
        ));
    }
    Ok(PathBuf::from(path))
}

#[cfg(test)]
#[path = "workspace_policy.tests.rs"]
mod tests;
