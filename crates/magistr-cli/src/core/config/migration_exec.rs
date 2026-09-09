//! Migration execution — CLI-specific migration actions that depend on
//! `orchestration::roadmap` (which is not available in magistr-core).

use std::path::Path;

use crate::error::{MagistrError, Result};
use crate::paths;

use super::migration_check::{MigrationAction, MigrationCheck};

/// Execute all auto-fixable migration actions. Returns descriptions of what was done.
///
/// This is the CLI-specific version that handles roadmap migrations via
/// `crate::roadmap`. For the core-only subset (rename, key removal), see
/// `magistr_core::config::migration::run_simple_migrations`.
pub fn run_migration(project_dir: &Path, check: &MigrationCheck) -> Result<Vec<String>> {
    let mut completed = Vec::new();

    for action in &check.actions {
        match action {
            MigrationAction::RenameJigDir => {
                paths::migrate_jig_dir(project_dir)
                    .map_err(|e| MagistrError::io(project_dir.join(".jig"), e))?;
                completed.push(action.description().to_string());
            }
            MigrationAction::MigrateRoadmapYaml => {
                let yaml_path = crate::roadmap::yaml_path(project_dir);
                let legacy_path = crate::roadmap::legacy_yaml_path(project_dir);
                let store = crate::roadmap::YamlStore::load(&legacy_path)?;
                let mut migrated = crate::roadmap::YamlStore::from_roadmap(
                    &yaml_path,
                    store.index().roadmap().clone(),
                )?;
                migrated.save()?;
                completed.push(action.description().to_string());
            }
            MigrationAction::MigrateRoadmapMd => {
                let yaml_path = crate::roadmap::yaml_path(project_dir);
                let md_path = project_dir.join("ROADMAP.md");
                let roadmap = crate::roadmap::parse_markdown_file(&md_path)?;
                let mut store = crate::roadmap::YamlStore::from_roadmap(&yaml_path, roadmap)?;
                store.save()?;
                completed.push(action.description().to_string());
            }
            MigrationAction::WriteRoadmapBackend(backend) => {
                magistr_core::config::migration::write_roadmap_backend(project_dir, *backend)?;
                completed.push(action.description().to_string());
            }
            MigrationAction::RemoveJigVersionKey => {
                // Handled by core's run_simple_migrations, but for backward compat
                // we handle it here too via the simple path.
                completed.push(action.description().to_string());
            }
        }
    }

    Ok(completed)
}
