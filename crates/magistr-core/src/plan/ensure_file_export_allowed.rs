//! Project-local policy for writing generated plan files.

use std::path::Path;

use serde::Deserialize;

use crate::error::{MagistrError, Result};

#[derive(Deserialize)]
struct ExportPolicy {
    #[serde(default = "exports_enabled")]
    plan_exports: bool,
}

fn exports_enabled() -> bool {
    true
}

/// Refuse file export when `.magistr/project.json` sets `plan_exports: false`.
/// Read on every request so an already-running server observes policy changes.
/// Missing configuration preserves existing behavior; unreadable or invalid
/// configuration refuses export instead of silently ignoring a possible opt-out.
pub fn ensure_file_export_allowed(project_root: &Path) -> Result<()> {
    let path = crate::paths::project_json(project_root)?;
    let raw = match std::fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(MagistrError::io(path, error)),
    };
    let policy: ExportPolicy = serde_json::from_str(&raw).map_err(|error| {
        MagistrError::Config(format!(
            "cannot read plan export policy at {}: {error}",
            path.display()
        ))
    })?;
    if !policy.plan_exports {
        return Err(MagistrError::Permission(format!(
            "plan file exports are disabled by plan_exports: false in {}; use native plan tools",
            path.display()
        )));
    }
    Ok(())
}

#[cfg(test)]
#[path = "ensure_file_export_allowed.tests.rs"]
mod tests;
