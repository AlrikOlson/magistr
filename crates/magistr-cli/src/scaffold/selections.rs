//! User + AI selections carried through the scaffold pipeline.

use super::research::{
    Recommendations, RecommendedFormat, RecommendedQualityGate, RecommendedTesting,
};

/// The selections that drive scaffold generation.
///
/// Carries both user selections (from the init interview) and AI-discovered
/// recommendations (from `magistr_recommend` during explicit agent-assisted setup)
/// through the scaffold pipeline. All agents receive the same unified
/// conventions.
#[derive(Debug, Clone, Default)]
pub struct ScaffoldSelections {
    /// Project type (cli, web-app, api, library, monorepo, other)
    pub project_type: String,
    /// Project description (may be AI-suggested or user-entered)
    pub description: String,
    /// Selected AI agents to emit config for (e.g. "cursor", "copilot", "cline")
    pub selected_agents: Vec<String>,
    /// Selected MCP servers to include in .mcp.json (e.g. "context7", "playwright")
    pub selected_mcp_servers: Vec<String>,
    /// AI-discovered conventions and best practices for this stack.
    /// Merged with static language conventions during scaffolding.
    pub conventions: Vec<String>,
    /// Practical tips for configuring AI agents with this stack.
    pub tips: Vec<String>,
    /// AI-recommended quality gate commands (override static preset when present).
    pub quality_gate: Option<RecommendedQualityGate>,
    /// AI-recommended format configuration (override static preset when present).
    pub format: Option<RecommendedFormat>,
    /// AI-recommended testing configuration (override static preset when present).
    pub testing: Option<RecommendedTesting>,
    /// Reference repositories the AI found relevant.
    pub reference_repos: Vec<String>,
    /// AI-generated file contents that override static templates during scaffolding.
    /// Keyed by relative path from project root (e.g. `"CLAUDE.md"`).
    pub generated_files: std::collections::HashMap<String, String>,
    /// AI-recommended lifecycle hooks (from `magistr_recommend`).
    pub lifecycle_hooks: Option<Vec<magistr_engine::LifecycleHook>>,
    /// Adaptive interview answers (database, architecture, CI/CD, deployment).
    /// Wired into the preset layer system for layered preset composition.
    pub interview_answers: super::question_graph::InterviewAnswers,
}

impl ScaffoldSelections {
    /// Apply explicitly supplied setup choices to the resolved project configuration.
    pub(crate) fn apply_commands(
        &self,
        gates: &mut crate::types::QualityGate,
        format: &mut crate::types::FormatConfig,
        testing: &mut crate::types::TestingConfig,
    ) {
        if let Some(choices) = &self.quality_gate {
            if let Some(value) = &choices.check {
                gates.check.clone_from(value);
            }
            if let Some(value) = &choices.test {
                gates.test.clone_from(value);
            }
            if let Some(value) = &choices.lint {
                gates.lint.clone_from(value);
            }
        }
        if let Some(choices) = &self.format {
            if let Some(value) = &choices.command {
                format.command.clone_from(value);
            }
            if let Some(value) = &choices.extensions {
                format.extensions.clone_from(value);
            }
        }
        if let Some(choices) = &self.testing {
            if let Some(value) = &choices.companion_pattern {
                testing.companion_pattern.clone_from(value);
            }
            if let Some(value) = &choices.test_dir {
                testing.test_dir.clone_from(value);
            }
        }
    }

    /// Build selections from headless AI recommendations.
    ///
    /// Used by AI-assisted init paths to get `magistr_recommend` data into
    /// the scaffold without running the shell interview.
    pub fn from_recommendations(recs: Recommendations) -> Self {
        let lifecycle_hooks: Vec<_> = recs
            .lifecycle_hooks
            .iter()
            .filter_map(convert_recommended_hook)
            .collect();

        Self {
            conventions: recs.conventions,
            tips: recs.tips,
            quality_gate: recs.quality_gate,
            format: recs.format,
            testing: recs.testing,
            reference_repos: recs.reference_repos,
            generated_files: recs
                .generated_files
                .into_iter()
                .map(|f| (f.path, f.content))
                .collect(),
            selected_mcp_servers: recs.mcp_servers.iter().map(|s| s.key.clone()).collect(),
            selected_agents: recs.agents.iter().map(|a| a.key.clone()).collect(),
            description: recs.suggested_description.unwrap_or_default(),
            project_type: recs.suggested_project_type.unwrap_or_default(),
            lifecycle_hooks: (!lifecycle_hooks.is_empty()).then_some(lifecycle_hooks),
            interview_answers: super::question_graph::InterviewAnswers::new(),
        }
    }
}

/// Convert a recommended lifecycle hook from the AI into a real `LifecycleHook`.
///
/// Returns `None` if the event string is unparseable or neither `tool` nor
/// `command` is provided.
fn convert_recommended_hook(
    rec: &super::research::RecommendedLifecycleHook,
) -> Option<magistr_engine::LifecycleHook> {
    let event = crate::mcp_tools::hooks::parse_event(&rec.event)?;

    let action = if let Some(ref tool) = rec.tool {
        magistr_engine::HookAction::McpTool {
            tool: tool.clone(),
            arguments: serde_json::Value::Null,
        }
    } else {
        magistr_engine::HookAction::ShellCommand {
            command: rec.command.clone()?,
            timeout_secs: rec.timeout_secs,
        }
    };

    let on_failure = match rec.on_failure.as_deref() {
        Some("fail_fast") => magistr_engine::OnFailure::FailFast,
        Some("warn") => magistr_engine::OnFailure::Warn,
        _ => magistr_engine::OnFailure::Continue,
    };

    Some(magistr_engine::LifecycleHook {
        event,
        action,
        description: rec.description.clone(),
        on_failure,
    })
}

#[cfg(test)]
#[path = "selections.tests.rs"]
mod tests;
