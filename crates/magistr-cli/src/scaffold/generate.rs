use std::collections::HashMap;
use std::path::Path;

use crate::error::Result;

use super::preview::SetupPlan;
use super::{AdminService, ApplyReceipt, Expected, FileAction, ManifestOutcome, SetupSeeds};

/// Result of a generate/reinit operation, with per-file outcome tracking.
#[derive(Debug, Clone, Default)]
pub struct GenerateResult {
    /// Files that were created (did not exist before).
    pub created: Vec<String>,
    /// Files that were updated (existed but content changed).
    pub updated: Vec<String>,
    /// Files that were unchanged (existed with identical content).
    pub unchanged: Vec<String>,
    /// Files that were preserved (user customizations kept).
    pub preserved: Vec<String>,
    /// Retired files that were removed.
    pub removed: Vec<String>,
    /// Files the plan refused to touch.
    pub refused: Vec<String>,
    /// Files the plan meant to apply and could not, each with its cause.
    pub failed: Vec<(String, String)>,
    /// What happened to the scaffold manifest.
    pub manifest: ManifestOutcome,
}

impl GenerateResult {
    /// The result the receipt of a setup plan describes.
    fn from_receipt(receipt: &ApplyReceipt) -> Self {
        let mut result = Self {
            manifest: receipt.manifest.clone(),
            ..Self::default()
        };
        for change in &receipt.changes {
            if let Some(cause) = change
                .outcome
                .is_failed()
                .then(|| change.outcome.detail())
                .flatten()
            {
                result.failed.push((change.path.clone(), cause));
                continue;
            }
            let bucket = match change.action {
                FileAction::Create | FileAction::Overwrite
                    if change.expected == Expected::Absent =>
                {
                    &mut result.created
                }
                FileAction::Create | FileAction::Overwrite => &mut result.updated,
                FileAction::Unchanged => &mut result.unchanged,
                FileAction::Preserved => &mut result.preserved,
                FileAction::Remove => &mut result.removed,
                FileAction::Refused(_) => &mut result.refused,
            };
            bucket.push(change.path.clone());
        }
        result
    }

    /// All files that were written (created + updated).
    pub fn written(&self) -> Vec<String> {
        let mut all = self.created.clone();
        all.extend(self.updated.iter().cloned());
        all
    }

    /// All paths involved, collected as strings.
    pub fn all_paths(&self) -> Vec<String> {
        let mut all = self.created.clone();
        all.extend(self.updated.iter().cloned());
        all.extend(self.unchanged.iter().cloned());
        all.extend(self.preserved.iter().cloned());
        all.extend(self.removed.iter().cloned());
        all.extend(self.refused.iter().cloned());
        all.extend(self.failed.iter().map(|(path, _)| path.clone()));
        all
    }
}

/// Generate the full `.claude/` scaffold in `dir`.
///
/// Uses diff-based updating: skips unchanged files, preserves user customizations
/// for `PreserveCustomized` files (detected via manifest hash comparison), and
/// always overwrites magistr-internal files.
///
/// Returns the paths written. A setup that could not apply every change it
/// meant to, or could not save the manifest, is an error that names what
/// failed and what was applied before it; nothing is rolled back.
pub fn generate(
    dir: &Path,
    template_vars: &HashMap<String, String>,
    generated_overrides: &HashMap<String, String>,
    seeds: &SetupSeeds,
) -> Result<Vec<String>> {
    let (result, receipt) = generate_with_receipt(dir, template_vars, generated_overrides, seeds)?;
    match receipt.partial_error() {
        Some(error) => Err(error),
        None => Ok(result.written()),
    }
}

/// Generate scaffold with detailed result tracking.
///
/// Computes the setup plan [`preview`](super::preview) shows and executes
/// it through the [`AdminService`], so what a reviewer saw is what is
/// written; each change is revalidated before its write and a change that
/// cannot be applied is reported in `failed` while the rest still apply.
pub fn generate_with_result(
    dir: &Path,
    template_vars: &HashMap<String, String>,
    generated_overrides: &HashMap<String, String>,
    seeds: &SetupSeeds,
) -> Result<GenerateResult> {
    Ok(generate_with_receipt(dir, template_vars, generated_overrides, seeds)?.0)
}

fn generate_with_receipt(
    dir: &Path,
    template_vars: &HashMap<String, String>,
    generated_overrides: &HashMap<String, String>,
    seeds: &SetupSeeds,
) -> Result<(GenerateResult, ApplyReceipt)> {
    let SetupPlan {
        plan,
        vars,
        links,
        owned,
    } = SetupPlan::compute(dir, template_vars, generated_overrides, seeds)?;

    // All configuration and link preflights have passed. Detach legacy links
    // before creating directories or writing either host's generated files.
    let paths: Vec<&str> = plan
        .files
        .iter()
        .filter(|f| owned.contains(f.path.as_str()))
        .map(|f| f.path.as_str())
        .collect();
    let mut service = AdminService::open(dir, plan.operation, owned).with_links(links);
    service.migrate_links(&paths)?;
    for change in &plan.files {
        let receipt = service.execute(change);
        if receipt.outcome.is_failed() {
            tracing::warn!(target: "magistr", receipt = %receipt.describe(), "scaffold change not applied");
        }
    }
    if super::CodexSetup::selected(&vars) {
        service
            .manifest_mut()
            .codex
            .get_or_insert_default()
            .selected = true;
    }
    let receipt = service.finish();
    let mut result = GenerateResult::from_receipt(&receipt);
    write_generated_roadmap(dir, generated_overrides, &mut result);
    Ok((result, receipt))
}

/// If the AI generated a roadmap.yaml, write it ONLY if one doesn't already
/// exist. An existing roadmap.yaml is user/tool-created data — never
/// overwrite it. A think-and-ship project (the scaffolded default) keeps its
/// roadmap in the engines — Generate mode authors chunks there — so no YAML
/// is written.
fn write_generated_roadmap(
    dir: &Path,
    generated_overrides: &HashMap<String, String>,
    result: &mut GenerateResult,
) {
    if crate::roadmap::backend_for(dir).is_native() {
        return;
    }
    let Some(yaml_content) = generated_overrides.get(".magistr/roadmap.yaml") else {
        return;
    };
    let yaml_path = crate::roadmap::yaml_path(dir);
    if yaml_path.exists() {
        return;
    }
    if let Some(parent) = yaml_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    if std::fs::write(&yaml_path, yaml_content).is_ok() {
        result.created.push(".magistr/roadmap.yaml".to_string());
        if let Ok(mut store) = crate::roadmap::YamlStore::load(&yaml_path) {
            // save() regenerates ROADMAP.md from the YAML source
            let _ = store.save();
            if !result.created.contains(&"ROADMAP.md".to_string())
                && !result.updated.contains(&"ROADMAP.md".to_string())
            {
                result.updated.push("ROADMAP.md".to_string());
            }
        }
    }
}

#[cfg(test)]
#[path = "codex_lifecycle.tests.rs"]
mod codex_tests;

#[cfg(test)]
#[path = "generate.tests.rs"]
mod tests;
