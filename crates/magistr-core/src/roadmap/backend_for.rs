//! Resolve the roadmap backend from current shared project settings.

use std::path::Path;

use magistr_engine::native::RoadmapBackend;

use crate::error::Result;

/// The declared roadmap backend, or the native default when settings are absent.
///
/// Invalid links, unreadable files and invalid current settings return errors.
/// Roadmap and specification files never select a backend.
pub fn backend_for(project_dir: &Path) -> Result<RoadmapBackend> {
    let path = crate::paths::project_json(project_dir)?;
    Ok(crate::config::read_project_settings(&path)?
        .map(|config| config.roadmap_backend)
        .unwrap_or_default())
}

#[cfg(test)]
#[path = "backend_for.tests.rs"]
mod tests;
