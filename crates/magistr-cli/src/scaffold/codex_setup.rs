use super::{
    PlannedChange, codex_config::CodexConfig, managed_content::ManagedContent,
    manifest::ScaffoldManifest,
};
use crate::error::{MagistrError, Result};
use std::collections::HashMap;
use std::ops::Range;
use std::path::{Component, Path};

/// The Codex shared-file lifecycle. It never owns whole config or instruction files.
pub struct CodexSetup;

impl CodexSetup {
    pub(crate) fn is_present(dir: &Path) -> bool {
        ScaffoldManifest::load(dir)
            .codex
            .is_some_and(|o| o.selected)
    }

    pub(super) fn prepare_vars(dir: &Path, vars: &mut HashMap<String, String>) {
        if Self::is_present(dir) {
            let selected = vars.entry("selected_agents".into()).or_default();
            if selected.is_empty() {
                *selected = super::emitters::all_emitters()
                    .iter()
                    .filter(|e| e.id() != "codex")
                    .map(|e| e.id())
                    .collect::<Vec<_>>()
                    .join(",");
            }
            if !selected.split(',').any(|v| v == "codex") {
                if !selected.is_empty() {
                    selected.push(',');
                }
                selected.push_str("codex");
            }
        }
    }

    pub(super) fn selected(vars: &HashMap<String, String>) -> bool {
        vars.get("selected_agents")
            .is_some_and(|v| v.split(',').any(|s| s == "codex"))
    }

    pub(super) fn plan(
        dir: &Path,
        vars: &HashMap<String, String>,
        repair: bool,
    ) -> Result<Vec<PlannedChange>> {
        // Selection recovery reads this evidence too. A broken manifest must
        // never turn an existing target into an unowned default reinit.
        Self::validate_manifest(dir)?;
        if !Self::selected(vars) {
            return Ok(Vec::new());
        }
        let manifest = ScaffoldManifest::load(dir);
        let owned = manifest.codex.unwrap_or_default();
        let native_only = Self::native_only(dir)?;
        // Recognized legacy Claude links are migration inputs, not write targets.
        let links = super::codex_skill_links::CodexSkillLinks::capture(dir)?;
        for f in super::workflow_skills::WorkflowSkills::codex_files() {
            links.read(dir, f.path)?;
        }
        Self::read(dir, ".codex/run")?;
        let mut result = Vec::new();
        let config = Self::read(dir, CONFIG)?;
        if !repair || config.is_some() {
            let content = CodexConfig::merge(
                config.as_deref().unwrap_or_default(),
                &owned,
                vars,
                native_only,
                repair,
            )?;
            result.push(preview(CONFIG, config, content));
        }
        let agents = Self::read(dir, AGENTS)?;
        if !repair || agents.is_some() {
            let old = agents.as_deref().unwrap_or_default();
            let span = block_span(old)?;
            let generated = instructions(native_only);
            let mut content = old.to_owned();
            match span {
                Some(span)
                    if owned
                        .agents
                        .as_ref()
                        .is_some_and(|o| old[span.clone()] == o.generated) =>
                {
                    content.replace_range(span, &generated);
                }
                None if !repair => {
                    if !content.is_empty() && !content.ends_with('\n') {
                        content.push('\n');
                    }
                    content.push_str(&generated);
                }
                _ => {}
            }
            result.push(preview(AGENTS, agents, content));
        }
        let context = super::preview_context::PreviewContext::capture(dir)?;
        for f in &mut result {
            f.ownership_context = Some(context.clone());
        }
        Ok(result)
    }

    pub(super) fn native_only(dir: &Path) -> Result<bool> {
        let path = crate::paths::project_json(dir);
        let content = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(MagistrError::io(&path, e)),
        };
        let doc: serde_json::Value = serde_json::from_str(&content)?;
        Ok(doc
            .get("roadmap_exports")
            .and_then(serde_json::Value::as_bool)
            == Some(false))
    }

    pub(super) fn record(manifest: &mut ScaffoldManifest, f: &PlannedChange) -> Result<()> {
        if f.path == CONFIG {
            CodexConfig::record(
                f.existing_content.as_deref().unwrap_or_default(),
                &f.new_content,
                manifest.codex.get_or_insert_default(),
            )?;
        } else if f.path == AGENTS {
            let before = f.existing_content.as_deref().unwrap_or_default();
            let before = block_span(before)?.map(|r| before[r].to_owned());
            if let Some(span) = block_span(&f.new_content)? {
                let after = f.new_content[span].to_owned();
                let owned = manifest.codex.get_or_insert_default();
                if before.as_deref() != Some(&after) {
                    owned.agents = Some(ManagedContent::updated(
                        owned.agents.as_ref(),
                        before,
                        after,
                    ));
                }
                owned.selected = true;
            }
        }
        Ok(())
    }

    pub(super) fn is_shared(path: &str) -> bool {
        path == CONFIG || path == AGENTS
    }

    /// The repair a present Codex target needs: its shared components
    /// (repair mode, so nothing is added) and its owned files that exist,
    /// each judged against the manifest and the canonical inputs. Repair
    /// never creates; a customized file is preserved.
    pub(super) fn repair_plan(dir: &Path) -> Result<Vec<PlannedChange>> {
        if !Self::is_present(dir) {
            return Ok(Vec::new());
        }
        let vars = HashMap::from([("selected_agents".into(), "codex".into())]);
        let mut changes = Self::plan(dir, &vars, true)?;
        let links = super::codex_skill_links::CodexSkillLinks::capture(dir)?;
        let manifest = ScaffoldManifest::load(dir);
        let canonical = super::canonical::CanonicalSource::capture(dir);
        for f in super::scaffold_files(&vars)
            .into_iter()
            .filter(|f| Self::owned_path(f.path) && !links.covers(f.path))
        {
            let Some(current) = Self::read(dir, f.path)? else {
                continue;
            };
            let ownership = if canonical.is_canonical(f.path) {
                super::Ownership::Canonical
            } else {
                super::Ownership::judge(f.policy, f.path, Some(&current), &manifest)
            };
            let content = match canonical.resolve(f.path, f.content) {
                Ok(content) => content,
                Err(refusal) => {
                    changes.push(PlannedChange::refused(
                        f.path,
                        refusal,
                        Some(current),
                        ownership,
                    ));
                    continue;
                }
            };
            if manifest.is_customized(f.path, &current) {
                changes.push(PlannedChange::preserved(
                    f.path,
                    Some(current),
                    super::Ownership::Customized,
                ));
                continue;
            }
            changes.push(PlannedChange::merged(f.path, Some(current), content).owned_by(ownership));
        }
        Ok(changes)
    }

    /// The owned removal of a present Codex target: its shared components
    /// restored to the user's values and its recorded, unchanged skills and
    /// launcher removed; customized, unrecorded and non-regular paths are
    /// kept and say so. Native project state is never part of this plan.
    pub fn removal_plan(dir: &Path) -> Result<super::ChangePlan> {
        Self::validate_manifest(dir)?;
        let manifest = ScaffoldManifest::load(dir);
        let mut plan = super::ChangePlan::new(super::AdminOperation::Removal);
        let Some(owned) = manifest.codex.as_ref().filter(|o| o.selected) else {
            return Ok(plan);
        };
        plan.files.extend(Self::shared_removal_changes(dir, owned)?);
        let removal = super::owned_removal::OwnedRemoval::new(dir, &manifest);
        for path in Self::removal_paths(&manifest) {
            plan.files.extend(removal.plan(&path));
        }
        Ok(plan)
    }

    /// Remove only recorded unchanged Codex components and skills; retain
    /// native project state. The removal plan is applied through the
    /// [`AdminService`](super::AdminService): each removal is revalidated
    /// before it happens, and a path that changed since planning is left as
    /// found and reported. Answers the paths changed; a partial removal is
    /// an error that names what failed and what was removed before it.
    pub fn remove_target(dir: &Path) -> Result<Vec<String>> {
        let plan = Self::removal_plan(dir)?;
        let mut service = super::AdminService::open(dir, plan.operation, Vec::<String>::new());
        if !service
            .manifest()
            .codex
            .as_ref()
            .is_some_and(|o| o.selected)
        {
            return Ok(Vec::new());
        }
        for f in &plan.files {
            service.execute(f);
        }
        let retired = Self::removal_paths(service.manifest());
        let manifest = service.manifest_mut();
        for path in retired {
            manifest.files.remove(&path);
        }
        manifest.codex = None;
        let receipt = service.finish();
        match receipt.partial_error() {
            Some(error) => Err(error),
            None => Ok(receipt.applied_paths()),
        }
    }

    pub(super) fn mark_file(manifest: &mut ScaffoldManifest, path: &str) {
        if Self::owned_path(path) {
            let owned = manifest.codex.get_or_insert_default();
            owned.selected = true;
            owned.files.insert(path.to_owned());
        }
    }

    pub(super) fn removal_paths(manifest: &ScaffoldManifest) -> Vec<String> {
        manifest
            .codex
            .as_ref()
            .map(|o| {
                o.files
                    .iter()
                    .filter(|p| *p == ".codex/run" || p.starts_with(".agents/skills/"))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(super) fn owned_path(path: &str) -> bool {
        path == ".codex/run"
            || super::workflow_skills::WorkflowSkills::codex_files()
                .iter()
                .any(|f| f.path == path)
    }

    /// The shared components with magistr's entries removed and the user's
    /// original values restored. Both are prepared before either is written:
    /// malformed TOML/markers leave all artifacts intact.
    pub(super) fn shared_removal_changes(
        dir: &Path,
        owned: &super::codex_ownership::CodexOwnership,
    ) -> Result<Vec<PlannedChange>> {
        let mut changes = Vec::new();
        if let Some(old) = Self::read(dir, CONFIG)? {
            let new = CodexConfig::remove(&old, owned)?;
            changes.push(preview(CONFIG, Some(old), new));
        }
        if let Some(old) = Self::read(dir, AGENTS)? {
            let mut new = old.clone();
            if let Some(span) = block_span(&old)?
                && let Some(entry) = &owned.agents
                && old[span.clone()] == entry.generated
            {
                new.replace_range(span, entry.original.as_deref().unwrap_or_default());
            }
            changes.push(preview(AGENTS, Some(old), new));
        }
        Ok(changes)
    }

    pub(super) fn validate_manifest(dir: &Path) -> Result<()> {
        if let Some(content) = Self::read(dir, ".magistr/scaffold-manifest.json")? {
            serde_json::from_str::<ScaffoldManifest>(&content).map_err(|e| {
                MagistrError::Config(format!(
                    ".magistr/scaffold-manifest.json: {e}; ownership evidence left untouched"
                ))
            })?;
        }
        Ok(())
    }

    /// Refuse symlinks and nonregular targets, including ancestor directories.
    pub(super) fn read(dir: &Path, relative: &str) -> Result<Option<String>> {
        let mut path = dir.to_path_buf();
        let mut parts = Path::new(relative).components().peekable();
        while let Some(part) = parts.next() {
            let Component::Normal(name) = part else {
                return Err(MagistrError::Config("invalid scaffold path".into()));
            };
            path.push(name);
            match std::fs::symlink_metadata(&path) {
                Ok(m)
                    if (parts.peek().is_some() && !m.is_dir())
                        || (parts.peek().is_none() && !m.is_file()) =>
                {
                    return Err(MagistrError::Config(format!(
                        "{} must be a regular scaffold path without symlinks; left untouched",
                        path.display()
                    )));
                }
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(e) => return Err(MagistrError::io(&path, e)),
            }
        }
        std::fs::read_to_string(&path)
            .map(Some)
            .map_err(|e| MagistrError::io(&path, e))
    }
}

#[cfg(test)]
#[path = "codex_setup.tests.rs"]
mod tests;

const CONFIG: &str = ".codex/config.toml";
const AGENTS: &str = "AGENTS.md";
const START: &str = "<!-- magistr:codex -->";
const END: &str = "<!-- /magistr:codex -->";

fn block_span(doc: &str) -> Result<Option<Range<usize>>> {
    let starts: Vec<_> = doc.match_indices(START).map(|(i, _)| i).collect();
    let ends: Vec<_> = doc.match_indices(END).map(|(i, _)| i).collect();
    match (starts.as_slice(), ends.as_slice()) {
        ([], []) => Ok(None),
        ([start], [end]) if start < end => Ok(Some(*start..*end + END.len())),
        _ => Err(MagistrError::Config(
            "AGENTS.md: malformed or duplicate magistr:codex markers; left untouched".into(),
        )),
    }
}

fn instructions(native_only: bool) -> String {
    let policy = if native_only {
        "This project stores its roadmap only in native Magistr state. Never create or regenerate ROADMAP.md or ROADMAP.json, invoke roadmap_export, or redirect an export into the checkout. Preserve roadmap_exports=false and the project export-tool deny settings."
    } else {
        "The native roadmap is authoritative. Read .magistr/project.json before exporting a view: roadmap_exports=false forbids ROADMAP.md/ROADMAP.json and all file exports. Otherwise export only when the project requests a view."
    };
    format!(
        "{START}\n## Magistr in Codex\n\nLaunch with `magistr codex-launch --project-root . --` or `./.codex/run`. The native launcher resolves dependencies and applies the documented Codex skill-discovery compatibility boundary. Use `magistr codex-launch --project-root . --diagnose` for read-only diagnostics.\n\nUse the configured `magistr` MCP server for roadmap_*, think_*, ship_*, signal_* and magistr_* tools. It embeds think-and-ship; do not start a standalone workflow server. Use Workbench map/search/read/edit/files/run/script/undo for workspace content; follow .agents/skills/workbench-routing/SKILL.md. A held Workbench lock belongs to its existing connection: do not kill the owner or launch a competing probe.\n\nUse the nine project skills: $next, $next-loop, $validate, $roadmap, $roadmap-refresh, $signals, $craft, $init and $workbench-routing. $next runs one chunk in this Codex session. $next-loop is a bounded fresh-session handoff: finish one chunk, write the handoff, stop, and resume the bound only in a new Codex session. Read .magistr/project.json for commands and required custom gates and .claude/rules/conventions.md for shared project conventions. Claude hooks are additional enforcement only; Codex must explicitly run and record required gates.\n\n{policy}\n\nPreserve existing objectives and worktree changes. Follow the requested scope; setup, review and planning do not authorize starting a roadmap chunk. $next implements one ready chunk; $next-loop is bounded. Recovery, acceptance, recorded checks and an atomic commit are required before completing work.\n{END}"
    )
}

/// The shared Codex files are merged, never owned whole: the plan keeps the
/// user's values and instruction text around magistr's own entries.
fn preview(path: &str, old: Option<String>, new: String) -> PlannedChange {
    PlannedChange::merged(path, old, new)
}
