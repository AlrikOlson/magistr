use std::path::{Path, PathBuf};

use super::resolve_path::resolve_path;
use crate::contracts::{ErrorKind, WorkspaceError};

/// Immutable path restrictions shared by every workspace operation.
///
/// No approval or trust tier can override these restrictions. Paths are resolved
/// component by component, including dangling links and links before `..`.
/// This intentionally closes gaps in the pinned reference's lexical-first jail.
/// It does not close filesystem races; effect owners must revalidate admission.
pub struct JailPolicy {
    root: PathBuf,
    protected_paths: Vec<PathBuf>,
}

impl JailPolicy {
    pub fn new(root: impl AsRef<Path>) -> Result<Self, WorkspaceError> {
        let root = root.as_ref().canonicalize().map_err(|error| {
            WorkspaceError::new(ErrorKind::PathOutsideWorkspace, error.to_string())
        })?;
        if !root.is_dir() {
            return Err(outside("workspace root is not a directory"));
        }
        Ok(Self {
            root,
            protected_paths: Vec::new(),
        })
    }

    /// Add literal paths and their subtrees, including loaded configuration.
    /// Glob characters are literal. Protection is case-insensitive on every
    /// platform. Ancestors protect the workspace subtree too; unrelated outside
    /// paths cannot match an admitted target.
    pub fn with_protected_paths(
        mut self,
        paths: impl IntoIterator<Item = impl AsRef<Path>>,
    ) -> Result<Self, WorkspaceError> {
        for path in paths {
            let path = self.root.join(path.as_ref());
            let canonical = resolve_path(&path)?;
            self.protected_paths.push(canonical);
            // Retain the configured spelling too, so a changed symlink
            // cannot move the protected config target out of this policy.
            self.protected_paths.push(path);
        }
        Ok(self)
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn check_read(&self, candidate: impl AsRef<Path>) -> Result<PathBuf, WorkspaceError> {
        let canonical = resolve_path(&self.root.join(candidate))?;
        if !is_within(&self.root, &canonical, cfg!(windows)) {
            return Err(outside("path resolves outside the workspace root"));
        }
        Ok(canonical)
    }

    pub fn check_write(&self, candidate: impl AsRef<Path>) -> Result<PathBuf, WorkspaceError> {
        let canonical = self.check_read(candidate)?;
        for directory in [".git", ".workbench"] {
            let protected = self.root.join(directory);
            if is_within(&protected, &canonical, true)
                || is_within(&resolve_path(&protected)?, &canonical, true)
            {
                return Err(protected_error(directory));
            }
        }
        let config = self.root.join("workbench.toml");
        if equal_paths(&canonical, &config, true)
            || equal_paths(&canonical, &resolve_path(&config)?, true)
        {
            return Err(protected_error("workbench.toml"));
        }
        for protected in &self.protected_paths {
            if is_within(protected, &canonical, true)
                || is_within(&resolve_path(protected)?, &canonical, true)
            {
                return Err(protected_error(&protected.display().to_string()));
            }
        }
        Ok(canonical)
    }
}

fn is_within(root: &Path, candidate: &Path, case_fold: bool) -> bool {
    let mut candidate = candidate.components();
    root.components().all(|expected| {
        candidate.next().is_some_and(|actual| {
            if case_fold {
                expected.as_os_str().to_string_lossy().to_lowercase()
                    == actual.as_os_str().to_string_lossy().to_lowercase()
            } else {
                expected == actual
            }
        })
    })
}

fn equal_paths(left: &Path, right: &Path, case_fold: bool) -> bool {
    left.components().count() == right.components().count() && is_within(left, right, case_fold)
}

fn outside(message: &str) -> WorkspaceError {
    WorkspaceError::new(ErrorKind::PathOutsideWorkspace, message)
}

fn protected_error(path: &str) -> WorkspaceError {
    WorkspaceError::new(
        ErrorKind::ProtectedPath,
        format!("{path} is read-only to every mutating surface"),
    )
}

#[cfg(test)]
#[path = "jail_policy.tests.rs"]
mod tests;
