//! Bringing the scaffold magistr owns back up to date, without a re-init.

use std::collections::HashMap;
use std::path::Path;

use super::update_policy::UpdatePolicy;
use super::workbench_toml::WorkbenchSeed;
use super::workflow_skills::WorkflowSkills;
use crate::error::{MagistrError, Result};

/// Repair the scaffold artifacts magistr owns outright in `dir`, answering
/// the paths that were rewritten (empty when there was nothing to do).
///
/// # Why this exists, and why it is not `magistr init`
///
/// A fix that lands in a *generated* file — a new rule in `.claude/rules`,
/// a new key in `workbench.toml` — reaches a project only when somebody
/// re-runs `magistr init` there. Nobody does, because init is an interview:
/// it asks questions, rewrites the user's own files under their own
/// policies, and is the wrong size of act for "the tool guide gained a
/// line". So the fix sits in the binary and never arrives, which is exactly
/// how `workbench-confirm-unattended` and the `autoApply` rule were shipped
/// to one project out of three. This runs on every session instead, costs a
/// read per file, and writes only what actually differs.
///
/// Four rules keep it safe, and each one is a thing it deliberately will
/// not do:
///
/// 1. **Only [`UpdatePolicy::Overwrite`] files.** That policy already says
///    "magistr internals that users should not edit" and a re-init already
///    rewrites them unconditionally — so repairing them is the existing
///    decision applied sooner, not a new one. Everything under
///    `PreserveCustomized` or `NeverOverwrite` is the user's and is never
///    touched by this sweep. The shared workflow runtime uses recorded
///    ownership to preserve customized content.
/// 2. **Only files whose content does not depend on the project.** The
///    scaffold is rendered *twice*, under two variable maps that differ on
///    every key the interview can set, and a file whose two renderings are
///    not byte-identical is skipped. Looking for a leftover `{{placeholder}}`
///    is NOT enough and the first version of this got it wrong: a file like
///    `.cursor/rules/conventions.mdc` is *computed* from `language` and
///    comes out fully substituted, so it looks static and is not — repairing
///    it with an empty map replaced a project's Rust conventions with the
///    generic default. Comparing two renderings catches exactly that,
///    because the thing being asked is "does any input reach this file",
///    which is a question about the function and not about its output.
/// 3. **Only files that already exist.** An absent file means the user
///    removed it, or that init never wrote it; creating one here would be
///    scaffolding a project that did not ask to be scaffolded. One bounded
///    exception installs the new runtime companion before upgrading existing
///    Claude workflow skills that depend on it. A previously recorded runtime
///    that the user removed stays removed, and dependent upgrades are skipped.
/// 4. **Only in a magistr project** — `.magistr/project.json` must be
///    there — and never fatally: a repair that cannot write is a warning,
///    because it must not be able to stop the thing it runs before.
#[must_use]
pub fn repair(dir: &Path) -> Vec<String> {
    if !dir.join(".magistr/project.json").is_file() {
        return Vec::new();
    }
    let mut repaired = Vec::new();
    let owned = owned_files();
    let runtime_ready = match ensure_workflow_runtime(dir, &owned, &mut repaired) {
        Ok(ready) => ready,
        Err(error) => {
            tracing::warn!(target: "magistr", %error, "could not prepare workflow runtime; skill upgrades skipped");
            false
        }
    };
    for (path, content) in owned {
        // The companion has its own conservative migration policy. Never let
        // the unconditional sweep overwrite a customized runtime afterward.
        if path == WORKFLOW_RUNTIME || (WorkflowSkills::is_claude_workflow(path) && !runtime_ready)
        {
            continue;
        }
        // A vendored skill in its home repository converges on the canonical
        // file's own bytes, never back on the embedded (markered) version —
        // otherwise every startup here re-breaks the byte-equality gate the
        // home repo holds its dogfood copy to.
        let content = super::canonical_content(dir, path).unwrap_or(content);
        let target = dir.join(path);
        // Rule 3: repair, never create.
        let Ok(current) = std::fs::read_to_string(&target) else {
            continue;
        };
        if current == content {
            continue;
        }
        match std::fs::write(&target, &content) {
            Ok(()) => repaired.push(path.to_owned()),
            Err(error) => {
                tracing::warn!(target: "magistr", %error, path, "could not repair");
            }
        }
    }
    // Additive and idempotent on its own terms, and it declares a key rather
    // than rewriting a file, so it is not part of the owned-file sweep.
    match WorkbenchSeed::ensure_confirm_mode(dir) {
        Ok(true) => repaired.push("workbench.toml".to_owned()),
        Ok(false) => {}
        Err(error) => tracing::warn!(target: "magistr", %error, "could not repair workbench.toml"),
    }
    match super::CodexSetup::repair(dir) {
        Ok(paths) => repaired.extend(paths),
        Err(error) => tracing::warn!(target: "magistr", %error, "could not repair Codex scaffold"),
    }
    repaired
}

const WORKFLOW_RUNTIME: &str = ".claude/skills/craft/references/runtime.md";

/// A dependency migration, not a general missing-file repair. Prepare the
/// runtime before any skill starts referring to it, preserving deleted or
/// customized companions and refusing malformed ownership or symlink paths.
fn ensure_workflow_runtime(
    dir: &Path,
    owned: &[(&str, String)],
    repaired: &mut Vec<String>,
) -> Result<bool> {
    let mut has_workflow = false;
    for (path, _) in owned
        .iter()
        .filter(|(path, _)| WorkflowSkills::is_claude_workflow(path))
    {
        has_workflow |= super::CodexSetup::read(dir, path)?.is_some();
    }
    if !has_workflow {
        return Ok(false);
    }
    super::CodexSetup::validate_manifest(dir)?;
    let mut manifest = super::manifest::ScaffoldManifest::load(dir);
    let current = super::CodexSetup::read(dir, WORKFLOW_RUNTIME)?;
    let content = owned
        .iter()
        .find(|(path, _)| *path == WORKFLOW_RUNTIME)
        .map(|(_, content)| content)
        .expect("shared workflow runtime is a project-independent scaffold artifact");
    match current.as_deref() {
        Some(current)
            if current == content || manifest.is_customized(WORKFLOW_RUNTIME, current) =>
        {
            return Ok(true);
        }
        None if manifest.files.contains_key(WORKFLOW_RUNTIME) => return Ok(false),
        _ => {}
    }
    let target = dir.join(WORKFLOW_RUNTIME);
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|error| MagistrError::io(parent, error))?;
    }
    if current.is_none() {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
            .map_err(|error| MagistrError::io(&target, error))?;
        file.write_all(content.as_bytes())
            .map_err(|error| MagistrError::io(&target, error))?;
    } else {
        std::fs::write(&target, content).map_err(|error| MagistrError::io(&target, error))?;
    }
    manifest.record(WORKFLOW_RUNTIME, content);
    manifest.save(dir)?;
    repaired.push(WORKFLOW_RUNTIME.to_owned());
    Ok(true)
}

/// Every key [`build_template_vars`](crate::orchestration) can set, so the
/// two probe renderings differ on all of them at once.
///
/// A key missing here is a key a file could be computed from without this
/// noticing, so `every_template_var_key_is_probed` holds the list against
/// the one the scaffold is really built with.
const VAR_KEYS: &[&str] = &[
    "project_name",
    "project_description",
    "project_type",
    "language",
    "framework",
    "package_manager",
    "test_framework",
    "linter",
    "formatter",
    "check_cmd",
    "test_cmd",
    "lint_cmd",
    "format_cmd",
    "format_extensions",
    "companion_pattern",
    "test_dir",
    "allowed_tools",
    "selected_agents",
    "selected_mcp_servers",
    "mcp_permissions",
    "ai_conventions",
    "ai_tips",
    "reference_repos",
];

/// The scaffold files magistr owns outright *and* renders identically for
/// every project — rules 1 and 2 above, as one list.
fn owned_files() -> Vec<(&'static str, String)> {
    let probe = |value: &str| -> HashMap<String, String> {
        VAR_KEYS
            .iter()
            .map(|k| ((*k).to_owned(), format!("{k}-{value}")))
            .collect()
    };
    let other: HashMap<&str, String> = super::scaffold_files(&probe("b"))
        .into_iter()
        .map(|f| (f.path, f.content))
        .collect();
    super::scaffold_files(&probe("a"))
        .into_iter()
        .filter(|f| f.policy == UpdatePolicy::Overwrite)
        // Byte-identical under two different projects ⇒ no input reaches it.
        .filter(|f| other.get(f.path) == Some(&f.content))
        .map(|f| (f.path, f.content))
        .collect()
}

#[cfg(test)]
#[path = "repair.tests.rs"]
mod tests;
