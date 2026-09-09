use crate::workflow::guards::GuardsConfig;
use serde::{Deserialize, Serialize};

use super::{CustomGate, FormatConfig, QualityGate, TestingConfig};

/// Full project configuration (written to .claude/project.json).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectConfig {
    pub project: ProjectMeta,
    pub stack: StackConfig,
    pub quality_gate: QualityGate,
    pub format: FormatConfig,
    pub spec: SpecConfig,
    pub testing: TestingConfig,
    pub reference_repos: Vec<String>,
    pub state_prefix: String,
    /// Custom quality gates beyond the standard check/test/lint triple.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_gates: Vec<CustomGate>,
    /// Workflow guard enable/disable configuration.
    #[serde(default, skip_serializing_if = "is_default_guards")]
    pub guards: GuardsConfig,
    /// Free-form project hints injected into the agent system prompt.
    ///
    /// Use for project-specific context the agent should always know:
    /// available tools, conventions, deployment targets, team preferences, etc.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hints: Vec<String>,
    /// Lifecycle hooks triggered at workflow phase transitions and events.
    ///
    /// Each hook binds a lifecycle event (e.g., entering a phase, iteration
    /// start/end, gate pass/fail) to an MCP tool invocation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lifecycle_hooks: Vec<magistr_engine::LifecycleHook>,
}

fn is_default_guards(guards: &GuardsConfig) -> bool {
    *guards == GuardsConfig::default()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectMeta {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default, rename = "type")]
    pub project_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StackConfig {
    pub language: String,
    pub framework: String,
    pub package_manager: String,
    pub test_framework: String,
    pub linter: String,
    pub formatter: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SpecConfig {
    pub file: String,
    pub unchecked_pattern: String,
}
