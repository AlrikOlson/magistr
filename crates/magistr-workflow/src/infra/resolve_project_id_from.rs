use std::path::Path;

use super::project_id::{
    IdSource, declared_identity_in, project_id_for_path, sanitize_project_name,
};

/// Resolve identity from a captured environment override and an explicit directory.
///
/// A usable captured override wins over the nearest repository identity file;
/// otherwise the existing canonical-path identity remains unchanged. This never
/// reads environment variables or obtains the process working directory.
/// `None` means the override was absent when the caller captured configuration.
#[must_use]
pub fn resolve_project_id_from(project_name: Option<&str>, cwd: &Path) -> (String, IdSource) {
    if let Some(raw) = project_name {
        let sanitized = sanitize_project_name(raw.trim());
        if !sanitized.is_empty() {
            return (sanitized, IdSource::Environment);
        }
    }
    let path = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
    if let Some(declared) = declared_identity_in(&path) {
        return (declared.id, IdSource::RepoFile);
    }
    (project_id_for_path(&path), IdSource::DerivedFromPath)
}

#[cfg(test)]
#[path = "resolve_project_id_from.tests.rs"]
mod tests;
