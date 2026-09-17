//! Selective file application from a scaffold preview.
//!
//! Given a set of approved [`PlannedChange`] entries, writes only those files
//! to disk. Used by the interactive approval flow in `--dry-run` mode.

use std::path::Path;

use super::{FileAction, PlannedChange};
use crate::error::{MagistrError, Result};

/// Apply only the approved files from a scaffold preview.
///
/// Checks every selected target against the captured preview before writing.
/// A stale target rejects the whole batch without creating files or directories.
/// Successful regular scaffold writes update the ownership manifest, so later
/// initialization can distinguish template upgrades from user edits.
///
/// Creates necessary directories and sets shell-script permissions (Unix only).
/// This preflight does not lock out concurrent writers or roll back IO failures.
///
/// Returns the list of paths that were written.
pub fn apply_approved(dir: &Path, files: &[&PlannedChange]) -> Result<Vec<String>> {
    let pending: Vec<_> = files
        .iter()
        .copied()
        .filter(|f| matches!(f.action, FileAction::Create | FileAction::Overwrite))
        .collect();
    if pending.is_empty() {
        return Ok(Vec::new());
    }

    super::CodexSetup::validate_manifest(dir)?;
    let mut preview_manifest = super::manifest::ScaffoldManifest::load(dir);
    if pending.iter().any(|file| file.ownership_context.is_some()) {
        let current = super::preview_context::PreviewContext::capture(dir)?;
        for file in &pending {
            if let Some(expected) = &file.ownership_context
                && expected != &current
            {
                return Err(MagistrError::Other(format!(
                    "Scaffold preview ownership or project policy is stale for {}. Regenerate the preview and review it again; no selected files were written.",
                    file.path
                )));
            }
        }
    }

    let links = if pending.iter().any(|file| file.ownership_context.is_some()) {
        super::codex_skill_links::CodexSkillLinks::capture(dir)?
    } else {
        super::codex_skill_links::CodexSkillLinks::default()
    };
    let selected: Vec<_> = pending.iter().map(|f| f.path.as_str()).collect();
    links.validate_selection(&selected)?;

    // Validate the entire selected batch before creating even a directory.
    // Reading errors are not evidence of absence.
    for f in &pending {
        let full = dir.join(&f.path);
        let observed = links.read(dir, &f.path)?;
        if super::CodexSetup::is_shared(&f.path) {
            super::CodexSetup::record(&mut preview_manifest, f)?;
        }
        match std::fs::symlink_metadata(&full) {
            Ok(metadata) if !metadata.is_file() && !links.covers(&f.path) => {
                return Err(MagistrError::Other(format!(
                    "Scaffold preview target {} is not a regular file. Review the target and regenerate the preview; no selected files were written.",
                    f.path
                )));
            }
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                return Err(MagistrError::io(&full, error));
            }
            _ => {}
        }
        let current = observed;
        let expected_presence = f.action == FileAction::Overwrite;
        if f.existing_content.is_some() != expected_presence || current != f.existing_content {
            return Err(MagistrError::Other(format!(
                "Scaffold preview is stale for {}. Regenerate the preview and review it again; no selected files were written.",
                f.path
            )));
        }
    }

    links.migrate(dir, &selected)?;

    // Only regular scaffold files have whole-file ownership. Merged shared
    // files such as .mcp.json and .gitignore keep their own merge policies.
    let owned: std::collections::HashSet<_> =
        super::scaffold_files(&std::collections::HashMap::new())
            .into_iter()
            .map(|f| f.path)
            .collect();
    let mut owned = owned;
    owned.extend(
        super::workflow_skills::WorkflowSkills::codex_files()
            .into_iter()
            .map(|f| f.path),
    );
    owned.insert(".codex/run");
    let mut manifest = super::manifest::ScaffoldManifest::load(dir);
    let mut written = Vec::new();
    for f in pending {
        let full = dir.join(&f.path);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).map_err(|e| MagistrError::io(parent, e))?;
        }
        std::fs::write(&full, &f.new_content).map_err(|e| MagistrError::io(&full, e))?;

        if super::CodexSetup::is_shared(&f.path) {
            super::CodexSetup::record(&mut manifest, f)?;
            manifest.save(dir)?;
        }
        if super::CodexSetup::owned_path(&f.path) {
            super::CodexSetup::mark_file(&mut manifest, &f.path);
        }

        // Save each successful write's ownership even if a later write fails.
        if owned.contains(f.path.as_str()) {
            manifest.record(&f.path, &f.new_content);
            manifest.save(dir)?;
        }

        #[cfg(unix)]
        if super::is_executable_path(&f.path) {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o755);
            std::fs::set_permissions(&full, perms).map_err(|e| MagistrError::io(&full, e))?;
        }

        written.push(f.path.clone());
    }

    Ok(written)
}

#[cfg(test)]
#[path = "apply.tests.rs"]
mod tests;
