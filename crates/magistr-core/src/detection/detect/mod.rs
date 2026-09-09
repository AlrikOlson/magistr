mod ci;
pub mod confidence;
mod config_parser;
pub mod detection;
pub mod detector;
mod framework;
pub mod fs_detector;
pub mod hierarchy;
mod language;
mod package_manager;
mod project_name;
pub mod relationships;
pub mod role;
pub mod scanner;
pub mod scoring;
pub mod smart;
mod tools;
pub mod tree_builder;
pub mod wizard;
mod workspace;

pub use self::detector::Detector;
pub use self::fs_detector::FileSystemDetector;
pub use self::tree_builder::build_project_tree;

use std::path::Path;

use tracing::{debug, instrument};

use crate::error::{MagistrError, Result};
use crate::types::{PluginDetection, Stack};

pub use self::package_manager::detect_package_manager;
pub use self::project_name::detect_project_name;
pub use self::smart::detect_stack_smart;

/// Detect the formatter for one language in a directory.
///
/// Parses the directory's manifests itself, so callers outside this module
/// (which cannot see `ManifestData`) can ask the same question detection asks.
pub fn detect_formatter_in(
    dir: &Path,
    language: crate::types::Language,
) -> crate::types::Formatter {
    let manifest = config_parser::parse_manifests(dir);
    tools::detect_formatter(dir, language, &manifest)
}

/// Detect the full project stack from a directory.
///
/// Returns a `Stack` with `extra_detections` empty. Callers that need plugin
/// detections should populate `extra_detections` separately (the main CLI
/// crate does this via the plugin registry).
#[instrument(skip_all, fields(dir = %dir.display()))]
pub fn detect_stack(dir: &Path) -> Result<Stack> {
    detect_stack_with_plugins(dir, Vec::new())
}

/// Detect the full project stack, merging in pre-collected plugin detections.
#[instrument(skip_all, fields(dir = %dir.display()))]
pub fn detect_stack_with_plugins(
    dir: &Path,
    extra_detections: Vec<PluginDetection>,
) -> Result<Stack> {
    detect_stack_inner(dir, None, extra_detections)
}

/// Detect the project stack for one *given* language.
///
/// Everything downstream of the language ladder (package manager, framework,
/// tools, project name) already takes the language as a parameter — this is
/// the same detection with the first rung answered by the caller, used by the
/// tree builder when a directory carries manifests for several stacks.
pub(crate) fn detect_stack_as(dir: &Path, language: crate::types::Language) -> Result<Stack> {
    detect_stack_inner(dir, Some(language), Vec::new())
}

fn detect_stack_inner(
    dir: &Path,
    forced_language: Option<crate::types::Language>,
    extra_detections: Vec<PluginDetection>,
) -> Result<Stack> {
    let dir = dir.canonicalize().map_err(|e| MagistrError::io(dir, e))?;

    if !dir.is_dir() {
        return Err(MagistrError::DirNotFound(dir));
    }

    // Parse manifests once, pass structured data to detection functions
    let manifest = config_parser::parse_manifests(&dir);

    let language = forced_language.unwrap_or_else(|| language::detect_language(&dir));
    debug!(%language, "detected language");
    let package_manager = package_manager::detect_package_manager(&dir, language);
    debug!(%package_manager, "detected package manager");
    let framework = framework::detect_framework(&dir, language, &manifest);
    debug!(%framework, "detected framework");
    let test_framework = tools::detect_test_framework(&dir, language, &manifest);
    let linter = tools::detect_linter(&dir, language, &manifest);
    let formatter = tools::detect_formatter(&dir, language, &manifest);
    let project_name = project_name::detect_project_name(&dir, language);
    let workspace = workspace::detect_workspace(&dir);
    let ci_provider = ci::detect_ci_provider(&dir);

    let stack = Stack {
        project_name,
        language,
        package_manager,
        framework,
        test_framework,
        linter,
        formatter,
        workspace,
        ci_provider,
        extra_detections,
    };

    debug!(
        plugin_detections = stack.extra_detections.len(),
        "detection complete"
    );

    Ok(stack)
}

#[cfg(test)]
#[path = "mod.tests.rs"]
mod tests;
