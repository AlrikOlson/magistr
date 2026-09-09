//! Prompt builder for workflow agent sessions.
//!
//! Pure prompt construction lives in `magistr-engine`. This module provides
//! an adapter that constructs `PromptConfig` from `ProjectConfig`.

use super::spec::SpecSummary;
use crate::types::ProjectConfig;

// Re-export from engine.
pub use magistr_engine::prompt::PromptConfig;

/// Build the prompt for a `/next` iteration.
///
/// Adapter that converts `ProjectConfig` → `PromptConfig` and delegates
/// to the engine's prompt builder.
pub fn build_next_prompt(config: &ProjectConfig, spec_summary: &SpecSummary) -> String {
    let prompt_config = PromptConfig {
        spec_file: config.spec.as_ref().map(|spec| spec.file.clone()),
        check_command: config.quality_gate.check.clone(),
        test_command: config.quality_gate.test.clone(),
        lint_command: config.quality_gate.lint.clone(),
        format_command: config.format.command.clone(),
    };
    prompt_config.build_next_prompt(spec_summary)
}

#[cfg(test)]
#[path = "prompt.tests.rs"]
mod tests;
