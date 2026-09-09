use serde::Deserialize;

use super::{
    DiscoveredServer, GeneratedFile, RecommendedAgent, RecommendedFormat, RecommendedQualityGate,
    RecommendedTesting,
};

/// Structured recommendations parsed from the AI's JSON output.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Recommendations {
    /// Discovered MCP servers to offer in the wizard.
    #[serde(default)]
    pub mcp_servers: Vec<DiscoveredServer>,
    /// Recommended agents for this stack.
    #[serde(default)]
    pub agents: Vec<RecommendedAgent>,
    /// Conventions / best practices for this stack.
    #[serde(default)]
    pub conventions: Vec<String>,
    /// Practical tips for AI agent configuration.
    #[serde(default)]
    pub tips: Vec<String>,
    /// Suggested project description based on analysis.
    #[serde(default)]
    pub suggested_description: Option<String>,
    /// Suggested project type (cli, web-app, api, library, monorepo, other).
    #[serde(default)]
    pub suggested_project_type: Option<String>,
    /// AI-recommended quality gate commands (override static preset).
    #[serde(default)]
    pub quality_gate: Option<RecommendedQualityGate>,
    /// AI-recommended format configuration (override static preset).
    #[serde(default)]
    pub format: Option<RecommendedFormat>,
    /// AI-recommended testing configuration (override static preset).
    #[serde(default)]
    pub testing: Option<RecommendedTesting>,
    /// Reference repositories the AI found relevant.
    #[serde(default)]
    pub reference_repos: Vec<String>,
    /// AI-generated file contents (e.g. CLAUDE.md, ROADMAP.md, conventions.md).
    /// When present, these override the static templates during scaffolding.
    #[serde(default)]
    pub generated_files: Vec<GeneratedFile>,
    /// AI-recommended lifecycle hooks for this project.
    /// When present, these are merged with (or override) the language preset hooks.
    #[serde(default)]
    pub lifecycle_hooks: Vec<RecommendedLifecycleHook>,
    /// Confidence report: how confident the AI is in its recommendations
    /// and which are backed by encyclopedia vs. web research.
    #[serde(default)]
    pub confidence: Option<ConfidenceReport>,
}

/// Confidence report for the AI's recommendations.
///
/// Tracks which recommendations are grounded in the encyclopedia's built-in
/// knowledge vs. discovered through web research, plus an overall confidence level.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ConfidenceReport {
    /// Overall confidence level for the recommendation set.
    pub overall: ConfidenceLevel,
    /// Brief rationale explaining the confidence assessment.
    #[serde(default)]
    pub rationale: String,
    /// Recommendations backed by encyclopedia/preset data.
    #[serde(default)]
    pub encyclopedia_backed: Vec<String>,
    /// Recommendations discovered through web research.
    #[serde(default)]
    pub research_backed: Vec<String>,
}

/// Confidence level for a recommendation set.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConfidenceLevel {
    /// All key recommendations are grounded in encyclopedia + verified by research.
    High,
    /// Most recommendations are grounded; some areas rely on research alone.
    #[default]
    Medium,
    /// Significant areas lack encyclopedia coverage; recommendations are research-only.
    Low,
}

/// A lifecycle hook recommendation from the AI.
///
/// Simpler than the full `LifecycleHook` — uses string event/action types
/// that get parsed into the real types during scaffolding.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RecommendedLifecycleHook {
    /// Event string (e.g. "on_enter(verify)", "on_gate_pass").
    pub event: String,
    /// MCP tool name (mutually exclusive with `command`).
    #[serde(default)]
    pub tool: Option<String>,
    /// Shell command (mutually exclusive with `tool`).
    #[serde(default)]
    pub command: Option<String>,
    /// Timeout for shell commands (seconds).
    #[serde(default)]
    pub timeout_secs: Option<u64>,
    /// Optional description.
    #[serde(default)]
    pub description: Option<String>,
    /// Failure strategy: "continue", "fail_fast", or "warn".
    #[serde(default)]
    pub on_failure: Option<String>,
}
