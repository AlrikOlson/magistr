use std::collections::HashMap;
use std::path::Path;

use magistr_core::types::ProjectTree;

use crate::commands;
use crate::config;
use crate::config::inputs::ProjectInputs;
use crate::error::Result;
use crate::scaffold;
use crate::types::{Preset, Stack};

use super::selections::ScaffoldSelections;
use super::tree_to_config::TreeToConfigResult;

/// Encapsulates the preset → template vars → scaffold generation pipeline.
///
/// Eliminates duplication across init paths by centralizing the
/// load-preset → build-vars → generate/preview flow. A pipeline built from a
/// stack alone composes built-in preset layers only; [`Self::with_inputs`]
/// adds the user's preset override and the installed template plugins from
/// the host's captured inputs.
pub struct ScaffoldPipeline {
    pub stack: Stack,
    pub preset: Preset,
    pub selections: ScaffoldSelections,
    /// When set, the pipeline generates a multi-archetype project.json
    /// and per-archetype CLAUDE.md sections instead of the flat single-stack config.
    tree: Option<ProjectTree>,
    /// The host's captured inputs: user preset overrides and template plugins.
    inputs: Option<ProjectInputs>,
}

impl ScaffoldPipeline {
    /// Create a pipeline with default (non-AI) selections.
    ///
    /// Used by headless init paths (`cmd_init`, `cmd_init_smart`, `cmd_init_dry_run`)
    /// that don't run the interview.
    pub fn headless(stack: Stack) -> Self {
        let preset = load_preset(&stack, &ScaffoldSelections::default(), None);
        Self {
            stack,
            preset,
            selections: ScaffoldSelections::default(),
            tree: None,
            inputs: None,
        }
    }

    /// Create a pipeline with interview- or AI-enriched selections.
    ///
    /// Uses adaptive interview answers to load layered presets (database,
    /// CI/CD, deployment layers) when available.
    pub fn with_selections(stack: Stack, selections: ScaffoldSelections) -> Self {
        let preset = load_preset(&stack, &selections, None);
        Self {
            stack,
            preset,
            selections,
            tree: None,
            inputs: None,
        }
    }

    /// Compose with the host's captured inputs: the preset is reloaded with
    /// the user's override as its final layer, and installed template
    /// plugins are applied after generation.
    #[must_use]
    pub fn with_inputs(mut self, inputs: &ProjectInputs) -> Self {
        self.preset = load_preset(
            &self.stack,
            &self.selections,
            inputs.user_preset_dir().as_deref(),
        );
        self.inputs = Some(inputs.clone());
        self
    }

    /// The user's preset directory, when the host captured one.
    fn user_preset_dir(&self) -> Option<std::path::PathBuf> {
        self.inputs
            .as_ref()
            .and_then(ProjectInputs::user_preset_dir)
    }

    /// The multi-archetype configuration for a polyglot tree.
    fn archetypes(&self) -> Option<TreeToConfigResult> {
        self.tree.as_ref().map(|tree| {
            super::tree_to_config::tree_to_archetypes(tree, self.user_preset_dir().as_deref())
        })
    }

    /// Apply an explicit scaffold-target selection from the CLI.
    /// An omitted flag keeps the pipeline's interview or recommendation choices.
    #[must_use]
    pub fn with_agents(mut self, agents: &[String]) -> Self {
        if !agents.is_empty() {
            self.selections.selected_agents.clear();
            for agent in agents {
                if !self.selections.selected_agents.contains(agent) {
                    self.selections.selected_agents.push(agent.clone());
                }
            }
        }
        self
    }

    /// Create a pipeline for a polyglot repository.
    ///
    /// Uses the primary project's stack for the base scaffold, then overlays
    /// multi-archetype config derived from the full project tree.
    pub fn polyglot(tree: &ProjectTree) -> Self {
        // Use the first (primary) project's stack for the base template vars.
        let stack = if let Some(first) = tree.projects.first() {
            let abs_dir = tree.root.join(&first.path);
            crate::detect::detect_stack(&abs_dir).unwrap_or_default()
        } else {
            Stack::default()
        };

        let preset = load_preset(&stack, &ScaffoldSelections::default(), None);
        Self {
            stack,
            preset,
            selections: ScaffoldSelections::default(),
            tree: Some(tree.clone()),
            inputs: None,
        }
    }

    /// Build the template variable map.
    pub fn template_vars(&self) -> HashMap<String, String> {
        commands::misc::build_template_vars(&self.stack, &self.preset, &self.selections)
    }

    /// AI-generated file overrides from the selections.
    pub fn overrides(&self) -> &HashMap<String, String> {
        &self.selections.generated_files
    }

    /// Resolve lifecycle hooks: use AI-recommended hooks if available,
    /// otherwise fall back to the language preset.
    #[must_use]
    pub fn resolve_hooks(&self) -> Vec<magistr_engine::LifecycleHook> {
        if !self.selections.lifecycle_hooks.is_empty() {
            self.selections.lifecycle_hooks.clone()
        } else {
            magistr_engine::workflow::hook_presets::preset_for_language(
                &self.stack.language.to_string(),
            )
        }
    }

    /// Execute the full scaffold generation using the default scaffolder.
    pub fn generate(&self, dir: &Path) -> Result<Vec<String>> {
        self.generate_with(dir, &scaffold::DefaultScaffolder)
    }

    /// Execute scaffold generation with a custom scaffolder implementation.
    pub fn generate_with(
        &self,
        dir: &Path,
        scaffolder: &dyn scaffold::Scaffolder,
    ) -> Result<Vec<String>> {
        let is_new_config = !crate::paths::project_json(dir).exists();
        let vars = self.template_vars();
        let mut written = scaffolder.generate(dir, &vars, self.overrides())?;

        // Template plugins from the captured registry: the same plan the
        // preview shows, applied best-effort (plugin failures don't block
        // scaffolding; a destination that cannot be written is left out of
        // the answer and logged).
        if let Some(inputs) = &self.inputs {
            let plugins = scaffold::plan_template_plugins(dir, &vars, inputs);
            let mut service = scaffold::AdminService::open(
                dir,
                scaffold::AdminOperation::Setup,
                Vec::<String>::new(),
            );
            for change in &plugins {
                let receipt = service.execute(change);
                if receipt.outcome.is_failed() {
                    tracing::warn!(target: "magistr", receipt = %receipt.describe(), "template plugin not applied");
                }
            }
            written.extend(service.finish().applied_paths());
        }

        // Enrich a newly seeded config only. Once present, project.json is
        // user-owned: re-detection must not replace commands, hooks, topology
        // or fields this binary does not yet know how to deserialize.
        if is_new_config {
            let hooks = self.resolve_hooks();
            if !hooks.is_empty() {
                self.inject_hooks(dir, &hooks);
            }
            if let Some(polyglot) = self.archetypes() {
                self.inject_polyglot(dir, &polyglot);
                self.upsert_archetype_conventions(dir, &polyglot);
            }
        }

        // Declare the think-and-ship identity once, so the native engines, an
        // external think-and-ship server and every agent session key the same
        // store. Idempotent; never rewrites an existing declaration.
        if let Err(e) = magistr_engine::native::ensure_project_file(dir) {
            tracing::warn!("could not write .think-and-ship/project.json: {e}");
        }

        Ok(written)
    }

    /// Preview scaffold output without writing to disk, using the default scaffolder.
    pub fn preview(&self, dir: &Path) -> Result<scaffold::ChangePlan> {
        self.preview_with(dir, &scaffold::DefaultScaffolder)
    }

    /// Preview scaffold output with a custom scaffolder implementation. The
    /// plan includes the template plugins the host's captured inputs name,
    /// exactly as [`Self::generate_with`] applies them.
    pub fn preview_with(
        &self,
        dir: &Path,
        scaffolder: &dyn scaffold::Scaffolder,
    ) -> Result<scaffold::ChangePlan> {
        let vars = self.template_vars();
        let mut plan = scaffolder.preview(dir, &vars, self.overrides())?;
        if let Some(inputs) = &self.inputs {
            plan.files
                .extend(scaffold::plan_template_plugins(dir, &vars, inputs));
        }
        Ok(plan)
    }
}

impl ScaffoldPipeline {
    /// Whether this pipeline produces multi-archetype config.
    #[must_use]
    pub fn is_polyglot(&self) -> bool {
        self.tree.is_some()
    }

    /// Inject lifecycle hooks into the generated project.json.
    fn inject_hooks(&self, dir: &Path, hooks: &[magistr_engine::LifecycleHook]) {
        let config_path = crate::paths::project_json(dir);
        let Ok(content) = std::fs::read_to_string(&config_path) else {
            return;
        };
        let Ok(mut config) = serde_json::from_str::<crate::types::ProjectConfig>(&content) else {
            return;
        };
        config.lifecycle_hooks = hooks.to_vec();
        if let Ok(json) = serde_json::to_string_pretty(&config) {
            let _ = std::fs::write(&config_path, json);
        }
    }

    /// Patch project.json with multi-archetype config from the tree detection.
    fn inject_polyglot(&self, dir: &Path, polyglot: &TreeToConfigResult) {
        let config_path = crate::paths::project_json(dir);
        let Ok(content) = std::fs::read_to_string(&config_path) else {
            return;
        };
        let Ok(mut config) = serde_json::from_str::<crate::types::ProjectConfig>(&content) else {
            return;
        };

        config.archetypes = polyglot.archetypes.clone();
        config.topology = Some(polyglot.topology);
        config.relationships = polyglot.relationships.clone();

        if let Ok(json) = serde_json::to_string_pretty(&config) {
            let _ = std::fs::write(&config_path, json);
        }
    }

    /// Upsert a per-archetype conventions section into CLAUDE.md.
    ///
    /// Adds a `<!-- archetypes -->` … `<!-- /archetypes -->` block listing each
    /// archetype's language, root path, and key commands.
    fn upsert_archetype_conventions(&self, dir: &Path, polyglot: &TreeToConfigResult) {
        if polyglot.archetypes.is_empty() {
            return;
        }
        let path = dir.join("CLAUDE.md");
        let existing = std::fs::read_to_string(&path).unwrap_or_default();

        let section = build_archetype_section(&polyglot.archetypes, polyglot.topology);
        let updated = upsert_marked_section(
            &existing,
            "<!-- archetypes -->",
            "<!-- /archetypes -->",
            &section,
        );

        if updated != existing {
            let _ = std::fs::write(&path, updated);
        }
    }
}

/// Build the archetype section content for CLAUDE.md.
fn build_archetype_section(
    archetypes: &[magistr_core::types::Archetype],
    topology: magistr_core::types::Topology,
) -> String {
    let mut s = String::new();
    s.push_str("## Project Archetypes\n\n");

    let topo_label = match topology {
        magistr_core::types::Topology::SingleProject => "Single Project",
        magistr_core::types::Topology::FlatMonorepo => "Flat Monorepo",
        magistr_core::types::Topology::NestedFullstack => "Nested Fullstack",
        magistr_core::types::Topology::Workspace => "Workspace",
    };
    s.push_str(&format!("**Topology:** {topo_label}\n\n"));

    s.push_str("| Archetype | Language | Root | Check | Test | Lint |\n");
    s.push_str("|-----------|----------|------|-------|------|------|\n");

    for arch in archetypes {
        let fw = arch
            .framework
            .as_deref()
            .map(|f| format!(" + {f}"))
            .unwrap_or_default();
        s.push_str(&format!(
            "| {} | {}{} | `{}` | `{}` | `{}` | `{}` |\n",
            arch.id,
            arch.language,
            fw,
            arch.root.display(),
            arch.quality_gate.check,
            arch.quality_gate.test,
            arch.quality_gate.lint,
        ));
    }
    s
}

/// Upsert a marked section in a document.
///
/// If the markers exist, replaces the content between them.
/// If the markers don't exist, appends the block at the end.
fn upsert_marked_section(doc: &str, start_marker: &str, end_marker: &str, content: &str) -> String {
    let block = format!("{start_marker}\n{content}{end_marker}");

    if let Some(start) = doc.find(start_marker)
        && let Some(end) = doc[start..].find(end_marker)
    {
        let end_abs = start + end + end_marker.len();
        let mut result = String::with_capacity(doc.len());
        result.push_str(&doc[..start]);
        result.push_str(&block);
        result.push_str(&doc[end_abs..]);
        return result;
    }

    // Markers not found — append.
    let mut result = doc.to_string();
    if !result.ends_with('\n') {
        result.push('\n');
    }
    result.push('\n');
    result.push_str(&block);
    result.push('\n');
    result
}

/// Load the stack's preset: built-in layers, the interview's answer layers,
/// then the user's override from `user_preset_dir` when one was captured.
/// Answer layers that fail to compose fall back to the answerless
/// composition, and that to the default preset.
fn load_preset(
    stack: &Stack,
    selections: &ScaffoldSelections,
    user_preset_dir: Option<&Path>,
) -> Preset {
    let mut layers = selections.interview_answers.to_layer_options();
    layers.user_preset_dir = user_preset_dir.map(Into::into);
    let compose = |layers: &config::preset::LayerOptions| {
        config::preset::load_preset_layered(
            stack.language,
            stack.framework,
            stack.package_manager,
            layers,
        )
    };
    compose(&layers)
        .or_else(|_| {
            compose(&config::preset::LayerOptions {
                user_preset_dir: layers.user_preset_dir.clone(),
                ..Default::default()
            })
        })
        .unwrap_or_else(|_| config::preset::fallback_preset())
}

#[cfg(test)]
#[path = "pipeline.tests.rs"]
mod tests;
