//! Selective file application from a scaffold preview.
//!
//! Given a set of approved [`PlannedChange`] entries, writes only those files
//! to disk. Used by the interactive approval flow in `--dry-run` mode.

use std::path::Path;

use super::{AdminOperation, AdminService, ApplyReceipt, FileAction, PlannedChange};
use crate::error::{MagistrError, Result};

/// Apply only the approved changes from a scaffold preview.
///
/// A reviewer approved this batch as one preview, so the whole batch is
/// checked against the captured preview before anything is written: a
/// selected target that changed since rejects the batch without creating or
/// removing anything, and the preview is regenerated and reviewed again.
/// The approved creates, overwrites and removals are then applied through
/// the [`AdminService`], which revalidates each target once more at its own
/// write and answers a receipt per change; a change that fails is reported
/// in the receipt and the rest still apply. Successful regular scaffold
/// writes update the ownership manifest and an applied removal drops its
/// record, so later initialization can distinguish template upgrades from
/// user edits. A removal the reviewer did not approve is left in place.
pub fn apply_approved(dir: &Path, files: &[&PlannedChange]) -> Result<ApplyReceipt> {
    let pending: Vec<_> = files
        .iter()
        .copied()
        .filter(|f| {
            matches!(
                f.action,
                FileAction::Create | FileAction::Overwrite | FileAction::Remove
            )
        })
        .collect();
    if pending.is_empty() {
        return Ok(ApplyReceipt::new(AdminOperation::Setup));
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

    // Only regular scaffold files have whole-file ownership. Merged shared
    // files such as .mcp.json and .gitignore keep their own merge policies.
    let mut owned: std::collections::HashSet<_> =
        super::scaffold_files(&std::collections::HashMap::new())
            .into_iter()
            .map(|f| f.path)
            .collect();
    owned.extend(
        super::workflow_skills::WorkflowSkills::codex_files()
            .into_iter()
            .map(|f| f.path),
    );
    owned.insert(".codex/run");
    let mut service = AdminService::open(dir, AdminOperation::Setup, owned);

    // Validate the entire selected batch before creating even a directory.
    for f in &pending {
        if super::CodexSetup::is_shared(&f.path) {
            super::CodexSetup::record(&mut preview_manifest, f)?;
        }
        if let Err(failure) = service.revalidate(f) {
            return Err(MagistrError::Other(format!(
                "Scaffold preview is stale for {} ({failure}). Regenerate the preview and review it again; no selected files were written.",
                f.path
            )));
        }
    }

    for f in pending {
        service.execute(f);
    }
    Ok(service.finish())
}

#[cfg(test)]
#[path = "apply.tests.rs"]
mod tests;
