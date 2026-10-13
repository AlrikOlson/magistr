//! Agent role specialization — defines role archetypes with prompt templates,
//! tool policies, model preferences, and behavior constraints.
//!
//! Each `SpecializedRole` encapsulates everything needed to configure an agent
//! session for a specific task type (research, coding, review, testing, docs).

use std::fmt;

use serde::{Deserialize, Serialize};

// ─── Role Constraints ─────────────────────────────────────────────────────

/// Behavior constraints for a specialized agent role.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RoleConstraints {
    /// If true, the agent should not modify any files.
    pub read_only: bool,
    /// Maximum iterations/turns (0 = unlimited).
    pub max_iterations: u32,
}

// ─── Specialized Role ─────────────────────────────────────────────────────

/// A specialized agent role with prompt template, tool policies, and model preferences.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecializedRole {
    /// Unique identifier (e.g., "researcher", "coder").
    pub role_id: String,
    /// Human-readable display name.
    pub display_name: String,
    /// System prompt template. Use `{task}` as placeholder for the subtask description.
    pub system_prompt_template: String,
    /// Tools explicitly allowed (empty = all allowed).
    pub allowed_tools: Vec<String>,
    /// Tools explicitly denied (takes precedence over allowed).
    pub denied_tools: Vec<String>,
    /// Preferred model ID (e.g., "claude-haiku-4-5-20251001").
    pub model_preference: Option<String>,
    /// Fallback model chain if preferred is unavailable.
    pub model_fallback: Vec<String>,
    /// Behavior constraints.
    pub constraints: RoleConstraints,
    /// Icon character for display.
    pub icon: char,
    /// Color index into theme palette for TUI color-coding.
    pub color_index: u8,
    /// Optional archetype ID this role is specialized for.
    /// When set, this role carries archetype-specific conventions, tool policies,
    /// and system prompt context for a particular stack within a polyglot project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archetype_id: Option<String>,
}

impl SpecializedRole {
    /// Render the system prompt by substituting `{task}` with the actual task description.
    #[must_use]
    pub fn render_prompt(&self, task: &str) -> String {
        self.system_prompt_template.replace("{task}", task)
    }

    /// Get the effective model to use: preference first, then first fallback.
    #[must_use]
    pub fn effective_model(&self) -> Option<&str> {
        self.model_preference
            .as_deref()
            .or_else(|| self.model_fallback.first().map(String::as_str))
    }

    /// Get all 5 built-in role archetypes.
    #[must_use]
    pub fn built_in_roles() -> Vec<Self> {
        vec![
            Self::researcher(),
            Self::coder(),
            Self::reviewer(),
            Self::tester(),
            Self::documenter(),
        ]
    }

    /// Look up a built-in role by ID.
    #[must_use]
    pub fn built_in_by_id(id: &str) -> Option<Self> {
        Self::built_in_roles().into_iter().find(|r| r.role_id == id)
    }

    /// Human-readable description for each built-in role.
    #[must_use]
    pub fn description_for(role_id: &str) -> String {
        match role_id {
            "researcher" => {
                "Investigates codebases, gathers context, and reports findings".to_string()
            }
            "coder" => "Implements features, fixes bugs, and writes production code".to_string(),
            "reviewer" => {
                "Reviews code changes for quality, correctness, and best practices".to_string()
            }
            "tester" => "Writes and runs tests, verifies correctness and coverage".to_string(),
            "documenter" => "Creates and updates documentation, READMEs, and comments".to_string(),
            other => format!("Custom role: {other}"),
        }
    }

    // ─── Built-in Archetypes ──────────────────────────────────────────────

    /// Researcher: web search + workbench navigation, read-only, no edits.
    #[must_use]
    pub fn researcher() -> Self {
        Self {
            role_id: "researcher".into(),
            display_name: "Researcher".into(),
            system_prompt_template: concat!(
                "You are a research specialist. Your job is to investigate and analyze.\n",
                "Do NOT modify any files. Do NOT use Edit or Write tools.\n",
                "Use web search and the workbench-mcp tools (search, map, read) to gather ",
                "information; follow .claude/skills/workbench-routing/SKILL.md.\n",
                "Produce a detailed analysis with findings, relevant code locations, ",
                "and recommendations.\n\n",
                "Task: {task}",
            )
            .into(),
            allowed_tools: vec![
                "Read".into(),
                "Bash".into(),
                "mcp__workbench-mcp__search".into(),
                "mcp__workbench-mcp__map".into(),
                "mcp__workbench-mcp__read".into(),
                "mcp__workbench-mcp__workbench_status".into(),
                "mcp__brave-search__brave_web_search".into(),
            ],
            denied_tools: vec!["Edit".into(), "Write".into(), "NotebookEdit".into()],
            model_preference: Some("claude-haiku-4-5-20251001".into()),
            model_fallback: vec!["claude-sonnet-4-6".into()],
            constraints: RoleConstraints {
                read_only: true,
                max_iterations: 0,
            },
            icon: '\u{1f50d}', // 🔍
            color_index: 0,
            archetype_id: None,
        }
    }

    /// Coder: full tool access, TDD-focused prompt.
    #[must_use]
    pub fn coder() -> Self {
        Self {
            role_id: "coder".into(),
            display_name: "Coder".into(),
            system_prompt_template: concat!(
                "You are a coding specialist. Follow TDD: write a failing test first, ",
                "make it pass, then refactor.\n",
                "Follow project conventions. No unwrap() in library code.\n",
                "Navigate and edit with the workbench-mcp tools (search, map, read, edit, ",
                "files, run, script, undo); follow .claude/skills/workbench-routing/SKILL.md.\n",
                "Write clean, minimal code that solves the task.\n\n",
                "Task: {task}",
            )
            .into(),
            // All tools allowed — including every workbench-mcp tool.
            allowed_tools: vec![],
            denied_tools: vec![],
            model_preference: Some("claude-opus-4-6".into()),
            model_fallback: vec!["claude-sonnet-4-6".into()],
            constraints: RoleConstraints {
                read_only: false,
                max_iterations: 0,
            },
            icon: '\u{1f4bb}', // 💻
            color_index: 1,
            archetype_id: None,
        }
    }

    /// Reviewer: read-only, outputs structured critique.
    #[must_use]
    pub fn reviewer() -> Self {
        Self {
            role_id: "reviewer".into(),
            display_name: "Reviewer".into(),
            system_prompt_template: concat!(
                "You are a code review specialist. Do NOT modify any files.\n",
                "Navigate with the workbench-mcp tools (search, map, read); follow ",
                ".claude/skills/workbench-routing/SKILL.md.\n",
                "Review the code for:\n",
                "- Correctness and edge cases\n",
                "- Performance issues\n",
                "- Security vulnerabilities\n",
                "- Convention violations\n",
                "- Missing tests\n\n",
                "Output a structured critique with severity levels (critical, warning, info).\n\n",
                "Task: {task}",
            )
            .into(),
            allowed_tools: vec![
                "Read".into(),
                "Bash".into(),
                "mcp__workbench-mcp__search".into(),
                "mcp__workbench-mcp__map".into(),
                "mcp__workbench-mcp__read".into(),
                "mcp__workbench-mcp__workbench_status".into(),
            ],
            denied_tools: vec!["Edit".into(), "Write".into(), "NotebookEdit".into()],
            model_preference: Some("claude-sonnet-4-6".into()),
            model_fallback: vec!["claude-opus-4-6".into()],
            constraints: RoleConstraints {
                read_only: true,
                max_iterations: 0,
            },
            icon: '\u{1f50e}', // 🔎
            color_index: 2,
            archetype_id: None,
        }
    }

    /// Tester: test-writing focus, runs quality gates.
    #[must_use]
    pub fn tester() -> Self {
        Self {
            role_id: "tester".into(),
            display_name: "Tester".into(),
            system_prompt_template: concat!(
                "You are a test specialist. Focus on writing comprehensive tests.\n",
                "Write unit tests in {name}.tests.rs files alongside the module.\n",
                "Test error paths, not just happy paths. Use assert_eq!, assert_ne!, assert!.\n",
                "Run quality gates after writing tests to verify they pass.\n",
                "Navigate and edit with the workbench-mcp tools (search, read, edit, run); ",
                "follow .claude/skills/workbench-routing/SKILL.md.\n\n",
                "Task: {task}",
            )
            .into(),
            // All tools allowed — including every workbench-mcp tool.
            allowed_tools: vec![],
            denied_tools: vec![],
            model_preference: Some("claude-sonnet-4-6".into()),
            model_fallback: vec!["claude-opus-4-6".into()],
            constraints: RoleConstraints {
                read_only: false,
                max_iterations: 0,
            },
            icon: '\u{1f9ea}', // 🧪
            color_index: 3,
            archetype_id: None,
        }
    }

    /// Documenter: docs + comments, no logic changes.
    #[must_use]
    pub fn documenter() -> Self {
        Self {
            role_id: "documenter".into(),
            display_name: "Documenter".into(),
            system_prompt_template: concat!(
                "You are a documentation specialist. Add or update documentation.\n",
                "Write doc comments (/// or //!), update README files, and add inline\n",
                "comments where the logic is not self-evident.\n",
                "Do NOT change any logic or behavior — only document existing code.\n",
                "Navigate with workbench-mcp search/read and write docs with edit; ",
                "follow .claude/skills/workbench-routing/SKILL.md.\n\n",
                "Task: {task}",
            )
            .into(),
            allowed_tools: vec![], // Needs Edit/Write (and workbench edit) for docs
            denied_tools: vec![],
            model_preference: Some("claude-haiku-4-5-20251001".into()),
            model_fallback: vec!["claude-sonnet-4-6".into()],
            constraints: RoleConstraints {
                read_only: false,
                max_iterations: 0,
            },
            icon: '\u{1f4dd}', // 📝
            color_index: 4,
            archetype_id: None,
        }
    }
}

impl fmt::Display for SpecializedRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name)
    }
}

#[cfg(test)]
#[path = "role.tests.rs"]
mod tests;
