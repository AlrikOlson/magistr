//! Bringing the scaffold magistr owns back up to date, without a re-init.

use std::collections::HashMap;
use std::path::Path;

use super::canonical::CanonicalSource;
use super::manifest::ScaffoldManifest;
use super::update_policy::UpdatePolicy;
use super::workbench_toml::WorkbenchSeed;
use super::workflow_skills::WorkflowSkills;
use super::{AdminOperation, AdminService, ChangePlan, Ownership, PlannedChange, Refusal};
use crate::error::Result;

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
/// The repair is [planned](plan) first and the plan executed, so the same
/// judgement is reviewable without writing. Four rules keep it safe, and
/// each one is a thing it deliberately will not do:
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
///    that the user removed stays removed, and dependent upgrades are refused.
/// 4. **Only in a magistr project** — `.magistr/project.json` must be
///    there — and never fatally: a repair that cannot write is a warning,
///    because it must not be able to stop the thing it runs before.
///
/// In Magistr's own source checkout the owned workflow skills are rendered
/// from the live templates rather than the binary's embedded copies, so an
/// older installed binary cannot downgrade them; a template that is missing
/// or unreadable refuses the path instead.
#[must_use]
pub fn repair(dir: &Path) -> Vec<String> {
    let plan = plan(dir);
    // The runtime companion and the Codex target's files carry ownership
    // records; the project-independent sweep never did and still does not.
    let owned = plan
        .files
        .iter()
        .map(|change| change.path.as_str())
        .filter(|path| *path == WORKFLOW_RUNTIME || super::CodexSetup::owned_path(path));
    let mut service = AdminService::open(dir, plan.operation, owned);
    let mut runtime_ready = true;
    for change in &plan.files {
        let path = change.path.as_str();
        if WorkflowSkills::is_claude_workflow(path) && !runtime_ready {
            continue;
        }
        let receipt = service.execute(change);
        if receipt.outcome.is_failed() {
            tracing::warn!(target: "magistr", receipt = %receipt.describe(), "could not repair");
            if path == WORKFLOW_RUNTIME {
                // The companion the skills refer to did not land; leave
                // the skills at the version that does not need it.
                runtime_ready = false;
            }
        }
    }
    let receipt = service.finish();
    if receipt.manifest.is_failed() {
        tracing::warn!(target: "magistr", outcome = %receipt.manifest, "could not record the repair");
    }
    receipt.applied_paths()
}

/// The repair `dir` needs, without writing: every owned file that exists
/// and differs, the runtime companion migration, the `workbench.toml`
/// confirm mode and the Codex target's repair. An absent owned file is not
/// listed (rule 3); a path the plan will not touch is listed as refused.
#[must_use]
pub(crate) fn plan(dir: &Path) -> ChangePlan {
    let mut plan = ChangePlan::new(AdminOperation::Repair);
    if !dir.join(".magistr/project.json").is_file() {
        return plan;
    }
    let owned = owned_files();
    let canonical = CanonicalSource::capture(dir);
    let (runtime, runtime_ready) = match runtime_migration(dir, &owned, &canonical) {
        Ok(migration) => migration,
        Err(error) => {
            tracing::warn!(target: "magistr", %error, "could not prepare workflow runtime; skill upgrades skipped");
            (None, false)
        }
    };
    plan.files.extend(runtime);
    for (path, embedded) in owned {
        // The companion has its own conservative migration policy. Never let
        // the unconditional sweep overwrite a customized runtime afterward.
        if path == WORKFLOW_RUNTIME {
            continue;
        }
        plan.files
            .extend(owned_change(dir, path, embedded, &canonical, runtime_ready));
    }
    // Additive and idempotent on its own terms, and it declares a key rather
    // than rewriting a file, so it is not part of the owned-file sweep.
    plan.files.extend(confirm_mode_change(dir));
    match super::CodexSetup::repair_plan(dir) {
        Ok(changes) => plan.files.extend(changes),
        Err(error) => tracing::warn!(target: "magistr", %error, "could not plan Codex repair"),
    }
    plan
}

/// One owned file's repair: nothing when it is absent (rule 3), unchanged
/// or overwritten with the embedded or canonical rendering, refused when the
/// canonical input is unavailable or the runtime it depends on is not ready.
fn owned_change(
    dir: &Path,
    path: &'static str,
    embedded: String,
    canonical: &CanonicalSource,
    runtime_ready: bool,
) -> Option<PlannedChange> {
    let ownership = if canonical.is_canonical(path) {
        Ownership::Canonical
    } else {
        Ownership::Internal
    };
    let current = match super::read_target(dir, path) {
        Ok(None) => return None,
        Ok(Some(current)) => current,
        Err(refusal) => return Some(PlannedChange::refused(path, refusal, None, ownership)),
    };
    if WorkflowSkills::is_claude_workflow(path) && !runtime_ready {
        return Some(PlannedChange::refused(
            path,
            Refusal::MissingDependency {
                dependency: WORKFLOW_RUNTIME.to_owned(),
            },
            Some(current),
            ownership,
        ));
    }
    Some(match canonical.resolve(path, embedded) {
        Ok(content) => PlannedChange::merged(path, Some(current), content).owned_by(ownership),
        Err(refusal) => PlannedChange::refused(path, refusal, Some(current), ownership),
    })
}

const WORKFLOW_RUNTIME: &str = ".claude/skills/craft/references/runtime.md";

/// A dependency migration, not a general missing-file repair. Plan the
/// runtime before any skill starts referring to it, preserving deleted or
/// customized companions and refusing malformed ownership or symlink paths.
/// Answers the runtime's change (none when no Claude workflow skill is
/// present) and whether dependent skill upgrades may proceed.
fn runtime_migration(
    dir: &Path,
    owned: &[(&str, String)],
    canonical: &CanonicalSource,
) -> Result<(Option<PlannedChange>, bool)> {
    let mut has_workflow = false;
    for (path, _) in owned
        .iter()
        .filter(|(path, _)| WorkflowSkills::is_claude_workflow(path))
    {
        has_workflow |= super::CodexSetup::read(dir, path)?.is_some();
    }
    if !has_workflow {
        return Ok((None, false));
    }
    super::CodexSetup::validate_manifest(dir)?;
    let manifest = ScaffoldManifest::load(dir);
    let current = super::CodexSetup::read(dir, WORKFLOW_RUNTIME)?;
    let embedded = owned
        .iter()
        .find(|(path, _)| *path == WORKFLOW_RUNTIME)
        .map(|(_, content)| content.clone())
        .expect("shared workflow runtime is a project-independent scaffold artifact");
    let ownership = if canonical.is_canonical(WORKFLOW_RUNTIME) {
        Ownership::Canonical
    } else {
        Ownership::Internal
    };
    let content = match canonical.resolve(WORKFLOW_RUNTIME, embedded) {
        Ok(content) => content,
        Err(refusal) => {
            let refused = PlannedChange::refused(WORKFLOW_RUNTIME, refusal, current, ownership);
            return Ok((Some(refused), false));
        }
    };
    Ok(match current {
        Some(current) if manifest.is_customized(WORKFLOW_RUNTIME, &current) => (
            Some(PlannedChange::preserved(
                WORKFLOW_RUNTIME,
                Some(current),
                Ownership::Customized,
            )),
            true,
        ),
        Some(current) => (
            Some(
                PlannedChange::merged(WORKFLOW_RUNTIME, Some(current), content).owned_by(ownership),
            ),
            true,
        ),
        None if manifest.files.contains_key(WORKFLOW_RUNTIME) => (
            Some(PlannedChange::preserved(
                WORKFLOW_RUNTIME,
                None,
                Ownership::User,
            )),
            false,
        ),
        None => (
            Some(PlannedChange::merged(WORKFLOW_RUNTIME, None, content)),
            true,
        ),
    })
}

/// The `workbench.toml` confirm-mode declaration as a change: none when
/// there is no file, unchanged when it already declares one.
fn confirm_mode_change(dir: &Path) -> Option<PlannedChange> {
    match WorkbenchSeed::plan_confirm_mode(dir) {
        (Some(before), Some(body)) => {
            Some(PlannedChange::merged("workbench.toml", Some(before), body))
        }
        (Some(before), None) => Some(PlannedChange::merged(
            "workbench.toml",
            Some(before.clone()),
            before,
        )),
        (None, _) => None,
    }
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
