use magistr_engine::guard::GuardsConfig;
use magistr_engine::native::RoadmapBackend;
use serde::{Deserialize, Serialize};

use super::archetype::{Archetype, ArchetypeRelationship};
use super::project_tree::Topology;
use super::{CustomGate, FormatConfig, QualityGate, TestingConfig};

/// Full project configuration (written to .magistr/project.json).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectConfig {
    /// Project identity (name, description, type).
    pub project: ProjectMeta,
    /// Detected or configured technology stack.
    pub stack: StackConfig,
    /// Commands for check, test, and lint gates.
    pub quality_gate: QualityGate,
    /// Code formatter configuration.
    pub format: FormatConfig,
    /// Specification file tracking (roadmap).
    ///
    /// Absent on a think-and-ship project: the roadmap lives in the native
    /// engines and there is no YAML/markdown spec file to track.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spec: Option<SpecConfig>,
    /// Test runner and file placement configuration.
    pub testing: TestingConfig,
    /// URLs of reference repositories for style guidance.
    pub reference_repos: Vec<String>,
    /// Prefix for state files (e.g., `"my-project"` → `.magistr/my-project-*`).
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
    /// Which store backs the roadmap: `think-and-ship` (the native in-process
    /// engines, the default) or `yaml` (`.magistr-roadmap.yml`).
    /// An omitted field uses the native default, regardless of other files.
    #[serde(default)]
    pub roadmap_backend: RoadmapBackend,
    /// Whether generated roadmap files may be exported. Native reads remain available.
    #[serde(default = "default_roadmap_exports", skip_serializing_if = "is_true")]
    pub roadmap_exports: bool,
    /// Per-sub-project archetypes with their own commands, patterns, and conventions.
    ///
    /// When absent or empty, the project is treated as a single-archetype project
    /// and a synthetic archetype is derived from the top-level `stack`/`quality_gate`/
    /// `format`/`testing` fields via [`ProjectConfig::effective_archetypes`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub archetypes: Vec<Archetype>,
    /// Repository topology (mirrors [`Topology`] from the detection engine).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topology: Option<Topology>,
    /// Inter-archetype relationships.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relationships: Vec<ArchetypeRelationship>,
}

impl ProjectConfig {
    /// Return the effective archetype list.
    ///
    /// If `archetypes` is non-empty, returns it directly. Otherwise synthesizes
    /// a single archetype from the flat fields (`stack`, `quality_gate`,
    /// `format`, `testing`). This describes the current resolver, not a requirement
    /// to retain older configuration formats.
    #[must_use]
    pub fn effective_archetypes(&self) -> Vec<Archetype> {
        if !self.archetypes.is_empty() {
            return self.archetypes.clone();
        }
        vec![Archetype {
            id: "default".into(),
            name: self.project.name.clone(),
            language: self.stack.language.clone(),
            framework: if self.stack.framework == "none" {
                None
            } else {
                Some(self.stack.framework.clone())
            },
            root: std::path::PathBuf::from("."),
            role: super::ProjectRole::Unknown,
            quality_gate: self.quality_gate.clone(),
            format: self.format.clone(),
            testing: self.testing.clone(),
            source_patterns: vec![],
            test_patterns: vec![],
            config_patterns: vec![],
            naming_convention: None,
            module_structure: None,
            model_preference: None,
        }]
    }

    /// Find an archetype by its ID slug.
    #[must_use]
    pub fn find_archetype(&self, id: &str) -> Option<&Archetype> {
        self.archetypes.iter().find(|a| a.id == id)
    }
}

#[cfg(test)]
#[path = "project_config.tests.rs"]
mod tests;

fn default_roadmap_exports() -> bool {
    true
}

fn is_true(value: &bool) -> bool {
    *value
}

fn is_default_guards(guards: &GuardsConfig) -> bool {
    *guards == GuardsConfig::default()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectMeta {
    /// Project name (used for display and state prefix).
    pub name: String,
    /// Short project description.
    #[serde(default)]
    pub description: String,
    /// Project archetype: `"cli"`, `"web"`, `"library"`, `"api"`, etc.
    #[serde(default, rename = "type")]
    pub project_type: String,
}

/// Technology stack configuration — language, framework, and toolchain.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StackConfig {
    /// Primary language: `"rust"`, `"typescript"`, `"python"`, etc.
    pub language: String,
    /// Web/app framework: `"none"`, `"nextjs"`, `"django"`, etc.
    pub framework: String,
    /// Package manager: `"cargo"`, `"npm"`, `"pip"`, etc.
    pub package_manager: String,
    /// Test runner: `"cargo-nextest"`, `"jest"`, `"pytest"`, etc.
    pub test_framework: String,
    /// Linter: `"clippy"`, `"eslint"`, `"ruff"`, etc.
    pub linter: String,
    /// Formatter: `"rustfmt"`, `"prettier"`, `"black"`, etc.
    pub formatter: String,
}

/// Specification file tracking configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SpecConfig {
    /// Path to the specification/roadmap file (e.g., `"ROADMAP.md"`).
    pub file: String,
    /// Regex pattern matching unchecked items (e.g., `"^\\s*- \\[ \\]"`).
    pub unchecked_pattern: String,
}
