//! Resolve the plan backend from current shared project settings.

use std::path::Path;

use magistr_engine::native::PlanBackend;

use crate::error::Result;

/// The declared plan backend, or the native default when settings are absent.
///
/// Invalid links, unreadable files and invalid current settings return errors.
/// Plan and specification files never select a backend.
pub fn backend_for(project_dir: &Path) -> Result<PlanBackend> {
    let path = crate::paths::project_json(project_dir)?;
    Ok(crate::config::read_project_settings(&path)?
        .map(|config| config.plan_backend)
        .unwrap_or_default())
}

#[cfg(test)]
#[path = "backend_for.tests.rs"]
mod tests;
