//! The setup plan: what `generate()` would do, computed without writing.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::error::Result;

use super::canonical::CanonicalSource;
use super::codex_skill_links::CodexSkillLinks;
use super::manifest::ScaffoldManifest;
use super::{
    AdminOperation, ChangePlan, FileAction, Ownership, PlannedChange, ScaffoldFile, SetupSeeds,
};

/// The configuration a first init seeds and re-detection preserves.
const PROJECT_JSON: &str = ".magistr/project.json";

/// Preview what `generate()` would do without writing any files.
///
/// Returns the setup [`ChangePlan`]: every scaffold file, the shared merges
/// (`.mcp.json`, `.gitignore`, the think-and-ship section of `CLAUDE.md`,
/// the `workbench.toml` seed), the retired rule files setup removes, the
/// shared Codex components and, for a first init, the `seeds` planted in the
/// fresh configuration and the think-and-ship identity declaration, each
/// with its action, ownership and diff.
pub fn preview(
    dir: &Path,
    template_vars: &HashMap<String, String>,
    generated_overrides: &HashMap<String, String>,
    seeds: &SetupSeeds,
) -> Result<ChangePlan> {
    Ok(SetupPlan::compute(dir, template_vars, generated_overrides, seeds)?.plan)
}

/// The setup plan together with the inputs generation needs to execute it.
pub(super) struct SetupPlan {
    pub(super) plan: ChangePlan,
    /// The template variables after Codex selection recovery.
    pub(super) vars: HashMap<String, String>,
    /// Legacy Codex skill links to detach before any generated write.
    pub(super) links: CodexSkillLinks,
    /// The regular scaffold files: whole-file ownership recorded in the
    /// manifest. Merged shared files keep their own merge policies.
    pub(super) owned: HashSet<&'static str>,
}

impl SetupPlan {
    pub(super) fn compute(
        dir: &Path,
        template_vars: &HashMap<String, String>,
        generated_overrides: &HashMap<String, String>,
        seeds: &SetupSeeds,
    ) -> Result<Self> {
        super::existing_config::ExistingConfig::validate(dir)?;

        // Host configuration stays portable; the native launcher discovers the
        // workspace and dependencies when the MCP client starts it.
        let mut vars = template_vars.clone();
        super::CodexSetup::prepare_vars(dir, &mut vars);
        vars.entry("project_root".to_string())
            .or_insert_with(|| ".".to_string());
        // Invalid explicit argv must fail before any scaffold writes.
        super::mcp_config::build(&vars)?;

        let codex = super::CodexSetup::plan(dir, &vars, false)?;
        let codex_selected = super::CodexSetup::selected(&vars);
        let ownership_context = if codex_selected {
            Some(super::preview_context::PreviewContext::capture(dir)?)
        } else {
            None
        };
        let links = if codex_selected {
            CodexSkillLinks::capture(dir)?
        } else {
            CodexSkillLinks::default()
        };
        let canonical = CanonicalSource::capture(dir);
        let manifest = ScaffoldManifest::load(dir);
        let mut files = super::scaffold_files(&vars);
        apply_overrides(&mut files, generated_overrides);
        let owned: HashSet<&'static str> = files.iter().map(|f| f.path).collect();

        let mut plan = ChangePlan::new(AdminOperation::Setup);
        for file in &files {
            let mut change = plan_file(dir, file, &links, &canonical, &manifest)?;
            if file.path == PROJECT_JSON && change.action == FileAction::Create {
                change.new_content = seeds.project_json(&change.new_content);
            }
            if file.path == "CLAUDE.md" {
                with_managed_section(&mut change, |document| {
                    super::think_and_ship::with_claude_md_section(document, &vars)
                });
            }
            if super::CodexSetup::owned_path(file.path) {
                change.ownership_context = ownership_context.clone();
            }
            plan.files.push(change);
        }
        // The archetype section is planted beside a fresh configuration
        // only: an existing project.json is the user's, and so is the
        // document that describes it.
        if plan
            .get(PROJECT_JSON)
            .is_some_and(|config| config.action == FileAction::Create)
            && let Some(change) = plan.files.iter_mut().find(|c| c.path == "CLAUDE.md")
        {
            with_managed_section(change, |document| seeds.claude_md(document));
        }
        plan.files.push(super::mcp_json_change(dir, &vars)?);
        plan.files.push(super::gitignore_change(dir, &vars));
        plan.files.extend(workbench_change(dir, &vars));
        plan.files.extend(super::legacy_rule_removals(dir));
        plan.files.extend(codex);
        plan.files.extend(identity_change(dir));
        Ok(Self {
            plan,
            vars,
            links,
            owned,
        })
    }
}

/// Replace static template content with AI-generated overrides.
fn apply_overrides(files: &mut [ScaffoldFile], generated_overrides: &HashMap<String, String>) {
    if generated_overrides.is_empty() {
        return;
    }
    let scaffold_paths: HashSet<&str> = files.iter().map(|f| f.path).collect();
    let mut matched = 0usize;
    for f in files.iter_mut() {
        if let Some(content) = generated_overrides.get(f.path) {
            tracing::debug!(path = f.path, "Applied AI-generated file override");
            f.content = content.clone();
            matched += 1;
        }
    }
    for key in generated_overrides.keys() {
        if !scaffold_paths.contains(key.as_str()) {
            tracing::warn!(
                path = key.as_str(),
                "AI-generated override did not match any scaffold file path — content ignored"
            );
        }
    }
    tracing::debug!(
        total = generated_overrides.len(),
        matched,
        orphaned = generated_overrides.len() - matched,
        "Generated file override summary"
    );
}

/// One regular scaffold file's planned change.
fn plan_file(
    dir: &Path,
    file: &ScaffoldFile,
    links: &CodexSkillLinks,
    canonical: &CanonicalSource,
    manifest: &ScaffoldManifest,
) -> Result<PlannedChange> {
    let path = file.path;
    // A legacy Codex skill link is an upgrade input: its Claude source text
    // is what the generated file replaces.
    let existing = if links.covers(path) {
        links.read(dir, path)?
    } else {
        match super::read_target(dir, path) {
            Ok(existing) => existing,
            Err(refusal) => {
                let ownership = Ownership::judge(file.policy, path, Some(""), manifest);
                return Ok(PlannedChange::refused(path, refusal, None, ownership));
            }
        }
    };
    let ownership = if links.covers(path) {
        Ownership::Internal
    } else if canonical.is_canonical(path) && existing.is_some() {
        Ownership::Canonical
    } else {
        Ownership::judge(file.policy, path, existing.as_deref(), manifest)
    };
    // A vendored skill in its home repository, or a workflow skill in
    // Magistr's own checkout, is the canonical rendering itself; a canonical
    // input that cannot be read keeps the embedded copy off the path.
    let content = match canonical.resolve(path, file.content.clone()) {
        Ok(content) => content,
        Err(refusal) => return Ok(PlannedChange::refused(path, refusal, existing, ownership)),
    };
    let action = if links.covers(path) {
        FileAction::Overwrite
    } else {
        file.policy
            .action(path, existing.as_deref(), &content, manifest)
    };
    let new_content = if action == FileAction::Preserved {
        existing.as_deref().unwrap_or(&content).to_owned()
    } else {
        content.clone()
    };
    let diff = if let Some(description) = links.description(path) {
        Some(format!(
            "{description}{}",
            super::unified_diff(path, existing.as_deref().unwrap_or_default(), &content)
        ))
    } else if action == FileAction::Overwrite {
        existing
            .as_deref()
            .map(|old| super::unified_diff(path, old, &content))
    } else {
        None
    };
    Ok(PlannedChange {
        path: path.to_owned(),
        action,
        ownership,
        new_content,
        existing_content: existing,
        ownership_context: None,
        diff,
    })
}

/// `CLAUDE.md` is the user's file (never overwritten as a whole), but the
/// marked sections between magistr's markers — think-and-ship, and the
/// archetypes of a polyglot init — are magistr's: the plan shows the document
/// with that section written or refreshed by `render`, so the preview says
/// exactly what generation writes. Sections chain: each renders over the
/// document the plan holds so far.
fn with_managed_section(change: &mut PlannedChange, render: impl FnOnce(&str) -> String) {
    if matches!(change.action, FileAction::Refused(_)) {
        return;
    }
    let next = render(&change.new_content);
    match change.existing_content.as_deref() {
        None => {
            change.action = FileAction::Create;
            change.new_content = next;
        }
        Some(existing) if existing == next => {
            change.action = FileAction::Unchanged;
            change.new_content = next;
            change.diff = None;
        }
        Some(existing) => {
            change.action = FileAction::Overwrite;
            change.ownership = Ownership::Shared;
            change.diff = Some(super::unified_diff("CLAUDE.md", existing, &next));
            change.new_content = next;
        }
    }
}

/// The think-and-ship identity declaration as a planned change: preserved
/// when the root already declares one (it is the key of every store the
/// project holds), refused when the path is not a readable regular file,
/// created with the id resolution answers today when nothing declares one,
/// and absent when an ancestor's declaration already covers the root.
fn identity_change(dir: &Path) -> Option<PlannedChange> {
    use magistr_engine::native::{PROJECT_DIR, PROJECT_FILE};
    let path = format!("{PROJECT_DIR}/{PROJECT_FILE}");
    match super::read_target(dir, &path) {
        Err(refusal) => Some(PlannedChange::refused(
            &path,
            refusal,
            None,
            Ownership::User,
        )),
        Ok(Some(existing)) => Some(PlannedChange::preserved(
            &path,
            Some(existing),
            Ownership::User,
        )),
        Ok(None) => magistr_engine::native::identity_declaration(dir)
            .map(|body| PlannedChange::merged(&path, None, body).owned_by(Ownership::Unclaimed)),
    }
}

/// The `workbench.toml` seed as a planned change: none when the gates seed
/// nothing, unchanged when the file already says everything, otherwise the
/// body seeding writes.
fn workbench_change(dir: &Path, vars: &HashMap<String, String>) -> Option<PlannedChange> {
    let seed = super::WorkbenchSeed::from_template_vars(vars);
    if seed.is_empty() {
        return None;
    }
    match seed.plan(dir) {
        (before, Some(body)) => Some(PlannedChange::merged("workbench.toml", before, body)),
        (Some(before), None) => Some(PlannedChange::merged(
            "workbench.toml",
            Some(before.clone()),
            before,
        )),
        (None, None) => None,
    }
}
