//! Autonomous workflow engine for `magistr next` and `magistr next-loop`.
//!
//! Drives plan-execute-verify iterations by spawning Claude Agent SDK sessions
//! with specialized prompts derived from the project spec.
//!
//! Async runtime-dependent modules (`gate_runner`, `hook_dispatch`, `watcher`)
//! remain in the CLI crate — this module contains all pure domain logic.

pub mod clean_tree_guard;
pub mod context_builder;
pub mod durable_write;
pub mod format_result;
pub mod formatter;
pub mod gate_cache;
pub mod gate_guard;
pub mod gates_check;
pub mod guards;
pub mod iteration;
pub mod native_summary;
pub mod plan;
pub mod plan_guard;
pub mod plans;
pub mod process_alive;
pub mod process_start;
pub mod project_context;
pub mod prompt;
pub mod reconciliation;
pub mod rollback;
pub mod rollback_result;
pub mod runtime_handle;
pub mod spec;
pub mod spec_guard;
pub mod spec_parse;
pub mod spec_read;

use std::path::Path;

use magistr_engine::{NativeConfig, NativeReader};

use crate::error::{MagistrError, Result};
use crate::types::ProjectConfig;
use crate::validate;

/// Spec file assumed when project.json declares no `spec` block.
const DEFAULT_SPEC_FILE: &str = "ROADMAP.md";

/// Configuration for a workflow run.
#[derive(Debug, Clone)]
pub struct WorkflowConfig {
    /// The project directory.
    pub project_dir: std::path::PathBuf,
    /// The loaded project configuration.
    pub project_config: ProjectConfig,
    /// The spec file path (resolved from project config). Inert on a
    /// think-and-ship project — progress comes from the engines.
    pub spec_path: std::path::PathBuf,
    /// The native engines when the project runs think-and-ship and a caller
    /// opened them (before raw mode); `None` opens them lazily on demand.
    native: Option<NativeReader>,
}

impl WorkflowConfig {
    /// Load workflow configuration from a project directory.
    ///
    /// If `spec.file` in project.json points to `ROADMAP.md` but that file
    /// doesn't exist, falls back to `.magistr-roadmap.yml` (the new canonical path).
    /// A think-and-ship project declares no `spec` block at all; the path is
    /// then a placeholder that nothing reads.
    pub fn load(project_dir: &Path) -> Result<Self> {
        let project_config = validate::load_project_config(project_dir)?;
        let spec_file = project_config
            .spec
            .as_ref()
            .map_or(DEFAULT_SPEC_FILE, |spec| spec.file.as_str());
        let mut spec_path = project_dir.join(spec_file);

        // A think-and-ship project needs no spec file at all.
        if !project_config.roadmap_backend.is_native() && !spec_path.is_file() {
            // Transparent migration: old project.json still says "ROADMAP.md"
            // but the YAML canonical file exists at the new path.
            let yaml_path = crate::roadmap::yaml_path(project_dir);
            if yaml_path.is_file() {
                spec_path = yaml_path;
            } else {
                return Err(MagistrError::Detection(format!(
                    "Spec file not found: {}",
                    spec_path.display()
                )));
            }
        }

        Ok(Self {
            project_dir: project_dir.to_path_buf(),
            project_config,
            spec_path,
            native: None,
        })
    }

    /// Whether the project runs the think-and-ship roadmap.
    #[must_use]
    pub fn is_native(&self) -> bool {
        self.project_config.roadmap_backend.is_native()
    }

    /// Use a reader a caller already opened (before entering raw mode) for
    /// the native progress reads. Ignored on a yaml project.
    #[must_use]
    pub fn with_native(mut self, engines: Option<NativeReader>) -> Self {
        self.native = engines;
        self
    }

    /// The reader for a native project: the attached one, or a fresh open
    /// on the store the environment names (prints load banners on stderr —
    /// call this before raw mode). Progress is a read; no lease is taken.
    fn engines(&self) -> Option<NativeReader> {
        if !self.is_native() {
            return None;
        }
        Some(
            self.native
                .clone()
                .unwrap_or_else(|| NativeReader::open(&NativeConfig::from_env(&self.project_dir))),
        )
    }

    /// Progress as a spec summary.
    ///
    /// Think-and-ship projects read chunk progress from the engines and
    /// never open `spec.file`; yaml projects prefer the YAML store and fall
    /// back to markdown parsing for non-magistr projects.
    pub fn spec_summary(&self) -> Result<spec::SpecSummary> {
        if let Some(engines) = self.engines() {
            return Ok(native_summary::native_spec_summary(&engines));
        }
        spec::spec_summary_from_project(&self.project_dir, &self.spec_path)
    }

    /// Build the agent prompt for a `/next` iteration.
    pub fn build_prompt(&self) -> Result<String> {
        let summary = self.spec_summary()?;

        if summary.all_done() {
            return Err(MagistrError::Detection(
                "All spec items are already complete. Nothing to do.".into(),
            ));
        }

        Ok(prompt::build_next_prompt(&self.project_config, &summary))
    }

    /// Get the state prefix for loop state files.
    pub fn state_prefix(&self) -> &str {
        &self.project_config.state_prefix
    }

    /// Clean up workflow state files.
    pub fn cleanup(&self) -> Result<()> {
        plan::cleanup_state(&self.project_dir)
    }
}

#[cfg(test)]
#[path = "mod.tests.rs"]
mod tests;
