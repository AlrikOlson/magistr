use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::error::{MagistrError, Result};

use super::codex_skill_links::CodexSkillLinks;
use super::manifest::ScaffoldManifest;
use super::preview::SetupPlan;
use super::{FileAction, PlannedChange};

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
}

impl GenerateResult {
    /// All files that were written (created + updated).
    pub fn written(&self) -> Vec<String> {
        let mut all = self.created.clone();
        all.extend(self.updated.iter().cloned());
        all
    }

    /// All paths involved (for backward compat with code expecting Vec<String>).
    pub fn all_paths(&self) -> Vec<String> {
        let mut all = self.created.clone();
        all.extend(self.updated.iter().cloned());
        all.extend(self.unchanged.iter().cloned());
        all.extend(self.preserved.iter().cloned());
        all.extend(self.removed.iter().cloned());
        all.extend(self.refused.iter().cloned());
        all
    }
}

/// Generate the full `.claude/` scaffold in `dir`.
///
/// Uses diff-based updating: skips unchanged files, preserves user customizations
/// for `PreserveCustomized` files (detected via manifest hash comparison), and
/// always overwrites magistr-internal files.
///
/// Returns a `GenerateResult` with detailed per-file outcomes.
pub fn generate(
    dir: &Path,
    template_vars: &HashMap<String, String>,
    generated_overrides: &HashMap<String, String>,
) -> Result<Vec<String>> {
    let result = generate_with_result(dir, template_vars, generated_overrides)?;
    Ok(result.written())
}

/// Generate scaffold with detailed result tracking.
///
/// Computes the setup plan [`preview`](super::preview) shows and executes
/// it change by change, so what a reviewer saw is what is written. This
/// preflights the plan once; it does not lock out concurrent writers or
/// revalidate each target at write time.
pub fn generate_with_result(
    dir: &Path,
    template_vars: &HashMap<String, String>,
    generated_overrides: &HashMap<String, String>,
) -> Result<GenerateResult> {
    let SetupPlan {
        plan,
        vars,
        links,
        owned,
    } = SetupPlan::compute(dir, template_vars, generated_overrides)?;

    // All configuration and link preflights have passed. Detach legacy links
    // before creating directories or writing either host's generated files.
    let paths: Vec<&str> = plan
        .files
        .iter()
        .filter(|f| owned.contains(f.path.as_str()))
        .map(|f| f.path.as_str())
        .collect();
    links.migrate(dir, &paths)?;

    let mut manifest = ScaffoldManifest::load(dir);
    let mut result = GenerateResult::default();
    for change in &plan.files {
        execute(dir, change, &owned, &links, &mut manifest, &mut result)?;
    }

    write_generated_roadmap(dir, generated_overrides, &mut result);

    if super::CodexSetup::selected(&vars) {
        manifest.codex.get_or_insert_default().selected = true;
    }
    manifest.save(dir)?;

    Ok(result)
}

/// Execute one planned change and record it.
fn execute(
    dir: &Path,
    change: &PlannedChange,
    owned: &HashSet<&'static str>,
    links: &CodexSkillLinks,
    manifest: &mut ScaffoldManifest,
    result: &mut GenerateResult,
) -> Result<()> {
    let path = change.path.as_str();
    let full = dir.join(path);
    let is_owned = owned.contains(path);
    match &change.action {
        FileAction::Preserved => result.preserved.push(change.path.clone()),
        FileAction::Refused(refusal) => {
            tracing::warn!(target: "magistr", path, %refusal, "scaffold change refused");
            result.refused.push(change.path.clone());
        }
        FileAction::Unchanged => {
            if super::is_executable_path(path) {
                super::CodexSetup::ensure_executable(&full)?;
            }
            if is_owned {
                super::CodexSetup::mark_file(manifest, path);
                manifest.record(path, &change.new_content);
            }
            result.unchanged.push(change.path.clone());
        }
        FileAction::Remove => {
            if full.is_file() {
                let _ = std::fs::remove_file(&full);
            }
            result.removed.push(change.path.clone());
        }
        FileAction::Create | FileAction::Overwrite => {
            if let Some(parent) = full.parent() {
                std::fs::create_dir_all(parent).map_err(|e| MagistrError::io(parent, e))?;
            }
            std::fs::write(&full, &change.new_content).map_err(|e| MagistrError::io(&full, e))?;
            #[cfg(unix)]
            if super::is_executable_path(path) {
                use std::os::unix::fs::PermissionsExt;
                let perms = std::fs::Permissions::from_mode(0o755);
                std::fs::set_permissions(&full, perms).map_err(|e| MagistrError::io(&full, e))?;
            }
            // Shared Codex components use the exact same preflighted plan as
            // the preview; their component ownership is recorded per write.
            if super::CodexSetup::is_shared(path) {
                super::CodexSetup::record(manifest, change)?;
                manifest.save(dir)?;
            }
            // Record the hash of what we just wrote.
            if is_owned {
                super::CodexSetup::mark_file(manifest, path);
                manifest.record(path, &change.new_content);
            }
            if change.existing_content.is_some() || links.covers(path) {
                result.updated.push(change.path.clone());
            } else {
                result.created.push(change.path.clone());
            }
        }
    }
    Ok(())
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
