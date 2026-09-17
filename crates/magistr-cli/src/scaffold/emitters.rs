//! Agent-agnostic file emitters for multi-agent scaffolding.
//!
//! Each emitter generates configuration files for a specific AI coding agent
//! from the same canonical source data (template vars + shared rule content).
//! This lets `magistr init` set up a project for Claude Code, Cursor, Copilot,
//! Cline, Windsurf, Aider, and Codex — all from one command.

use std::collections::HashMap;
use std::path::Path;

use crate::error::Result;

use super::conventions::resolve_conventions;
use super::{ScaffoldFile, UpdatePolicy};

// Re-use shared rule content from the parent module.
use super::STATIC_WORKFLOW;
use super::tool_guide;

// ── Emitter trait ──────────────────────────────────────────────────────

/// An agent emitter produces scaffold files for a specific AI coding tool.
#[allow(dead_code)] // name() used in tests and future CLI output
pub(crate) trait AgentEmitter {
    /// Internal identifier matching interview selection keys (e.g. "cursor", "copilot").
    fn id(&self) -> &'static str;

    /// Human-readable agent name (e.g. "Cursor", "GitHub Copilot").
    fn name(&self) -> &'static str;

    /// Files this emitter would generate.
    fn files(&self, vars: &HashMap<String, String>) -> Vec<ScaffoldFile>;

    /// Paths to clean up on `magistr reset`. Directories are removed recursively
    /// only if empty after file deletion.
    fn removable_paths(&self) -> Vec<&'static str>;
}

/// Return all built-in emitters (excluding Claude + MCP which are handled separately).
pub(crate) fn all_emitters() -> Vec<Box<dyn AgentEmitter>> {
    vec![
        Box::new(CursorEmitter),
        Box::new(CopilotEmitter),
        Box::new(ClineEmitter),
        Box::new(WindsurfEmitter),
        Box::new(AiderEmitter),
        Box::new(super::codex_emitter::CodexEmitter),
    ]
}

// ── Shared helpers ─────────────────────────────────────────────────────

/// Build a unified project instructions document reusable across agents.
///
/// Contains: project header, quick-start commands, workflow rules, conventions.
/// When AI-discovered conventions are available, they're appended after the
/// static language conventions — giving agents both baseline and project-specific guidance.
fn build_instructions(vars: &HashMap<String, String>) -> String {
    let name = var(vars, "project_name", "Project");
    let desc = var(vars, "project_description", "");
    let check = var(vars, "check_cmd", "");
    let test = var(vars, "test_cmd", "");
    let lint = var(vars, "lint_cmd", "");
    let lang = var(vars, "language", "unknown");
    let framework = var(vars, "framework", "none");

    let mut doc = format!("# {name}\n\n");
    if !desc.is_empty() {
        doc.push_str(desc);
        doc.push_str("\n\n");
    }
    doc.push_str(&format!("**Stack:** {lang}"));
    if framework != "none" {
        doc.push_str(&format!(" + {framework}"));
    }
    doc.push_str("\n\n## Quick Start\n\n```bash\n");
    doc.push_str(&format!("{check} && {test} && {lint}\n"));
    doc.push_str("```\n\n");
    doc.push_str("## Workflow\n\n");
    doc.push_str(STATIC_WORKFLOW);
    doc.push_str("\n\n## Conventions\n\n");
    doc.push_str(&resolve_conventions(vars).full_document());

    doc.push_str("\n\n");
    doc.push_str(&tool_guide::build_tool_guide());

    doc
}

fn var<'a>(vars: &'a HashMap<String, String>, key: &str, default: &'a str) -> &'a str {
    vars.get(key).map(|s| s.as_str()).unwrap_or(default)
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => format!("{}{}", c.to_uppercase(), chars.as_str()),
    }
}

/// File extension globs for a given language (used by Cursor rule frontmatter).
fn language_globs(language: &str) -> Vec<&'static str> {
    match language {
        "rust" => vec!["**/*.rs"],
        "typescript" | "javascript" => vec!["**/*.ts", "**/*.tsx", "**/*.js", "**/*.jsx"],
        "python" => vec!["**/*.py"],
        "go" => vec!["**/*.go"],
        "java" => vec!["**/*.java"],
        "ruby" => vec!["**/*.rb"],
        "elixir" => vec!["**/*.ex", "**/*.exs"],
        "kotlin" => vec!["**/*.kt", "**/*.kts"],
        "swift" => vec!["**/*.swift"],
        "cpp" | "c++" => vec!["**/*.cpp", "**/*.hpp", "**/*.cc", "**/*.h"],
        "csharp" | "c#" => vec!["**/*.cs"],
        _ => vec!["**/*"],
    }
}

/// Format a Cursor `.mdc` rule file with YAML frontmatter.
fn mdc_rule(description: &str, globs: &[&str], content: &str) -> String {
    let globs_json: Vec<String> = globs.iter().map(|g| format!("\"{g}\"")).collect();
    format!(
        "---\ndescription: {description}\nglobs: [{globs}]\nalwaysApply: true\n---\n{content}",
        globs = globs_json.join(", ")
    )
}

// ── Cursor ─────────────────────────────────────────────────────────────

struct CursorEmitter;

impl AgentEmitter for CursorEmitter {
    fn id(&self) -> &'static str {
        "cursor"
    }

    fn name(&self) -> &'static str {
        "Cursor"
    }

    fn files(&self, vars: &HashMap<String, String>) -> Vec<ScaffoldFile> {
        let lang = var(vars, "language", "unknown");
        let globs = language_globs(lang);
        let conventions = resolve_conventions(vars).full_document();

        vec![
            ScaffoldFile {
                path: ".cursor/rules/workflow.mdc",
                content: mdc_rule(
                    "Plan-execute-verify workflow methodology",
                    &["**/*"],
                    STATIC_WORKFLOW,
                ),
                policy: UpdatePolicy::Overwrite,
            },
            ScaffoldFile {
                path: ".cursor/rules/conventions.mdc",
                content: mdc_rule(
                    &format!("{} coding conventions", capitalize(lang)),
                    &globs,
                    &conventions,
                ),
                policy: UpdatePolicy::Overwrite,
            },
            ScaffoldFile {
                path: ".cursor/rules/tools.mdc",
                content: mdc_rule(
                    "magistr MCP tool usage guide",
                    &["**/*"],
                    &tool_guide::build_tool_guide(),
                ),
                policy: UpdatePolicy::Overwrite,
            },
        ]
    }

    fn removable_paths(&self) -> Vec<&'static str> {
        vec![
            ".cursor/rules/workflow.mdc",
            ".cursor/rules/conventions.mdc",
            ".cursor/rules/tools.mdc",
        ]
    }
}

// ── GitHub Copilot ─────────────────────────────────────────────────────

struct CopilotEmitter;

impl AgentEmitter for CopilotEmitter {
    fn id(&self) -> &'static str {
        "copilot"
    }

    fn name(&self) -> &'static str {
        "GitHub Copilot"
    }

    fn files(&self, vars: &HashMap<String, String>) -> Vec<ScaffoldFile> {
        vec![ScaffoldFile {
            path: ".github/copilot-instructions.md",
            content: build_instructions(vars),
            policy: UpdatePolicy::Overwrite,
        }]
    }

    fn removable_paths(&self) -> Vec<&'static str> {
        vec![".github/copilot-instructions.md"]
    }
}

// ── Cline ──────────────────────────────────────────────────────────────

struct ClineEmitter;

impl AgentEmitter for ClineEmitter {
    fn id(&self) -> &'static str {
        "cline"
    }

    fn name(&self) -> &'static str {
        "Cline"
    }

    fn files(&self, vars: &HashMap<String, String>) -> Vec<ScaffoldFile> {
        vec![ScaffoldFile {
            path: ".clinerules",
            content: build_instructions(vars),
            policy: UpdatePolicy::Overwrite,
        }]
    }

    fn removable_paths(&self) -> Vec<&'static str> {
        vec![".clinerules"]
    }
}

// ── Windsurf ───────────────────────────────────────────────────────────

struct WindsurfEmitter;

impl AgentEmitter for WindsurfEmitter {
    fn id(&self) -> &'static str {
        "windsurf"
    }

    fn name(&self) -> &'static str {
        "Windsurf"
    }

    fn files(&self, vars: &HashMap<String, String>) -> Vec<ScaffoldFile> {
        vec![ScaffoldFile {
            path: ".windsurfrules",
            content: build_instructions(vars),
            policy: UpdatePolicy::Overwrite,
        }]
    }

    fn removable_paths(&self) -> Vec<&'static str> {
        vec![".windsurfrules"]
    }
}

// ── Aider ──────────────────────────────────────────────────────────────

struct AiderEmitter;

impl AgentEmitter for AiderEmitter {
    fn id(&self) -> &'static str {
        "aider"
    }

    fn name(&self) -> &'static str {
        "Aider"
    }

    fn files(&self, vars: &HashMap<String, String>) -> Vec<ScaffoldFile> {
        let lint = var(vars, "lint_cmd", "");
        let test = var(vars, "test_cmd", "");

        let content = format!(
            "# Aider configuration — generated by magistr\n\
             # https://aider.chat/docs/config/aider_conf.html\n\n\
             lint-cmd: \"{lint}\"\n\
             test-cmd: \"{test}\"\n\
             auto-lint: true\n\
             auto-test: true\n"
        );

        vec![ScaffoldFile {
            path: ".aider.conf.yml",
            content,
            policy: UpdatePolicy::Overwrite,
        }]
    }

    fn removable_paths(&self) -> Vec<&'static str> {
        vec![".aider.conf.yml"]
    }
}

// ── Removal helper ─────────────────────────────────────────────────────

/// Pass emitter candidates through the same ownership check as Claude files.
pub(crate) fn remove_emitter_files(
    removal: &mut super::owned_removal::OwnedRemoval<'_>,
) -> Result<()> {
    for emitter in all_emitters() {
        for path in emitter.removable_paths() {
            removal.remove(path)?;
        }
    }
    Ok(())
}

/// The agent ids a project already carries, judged by whether any of an
/// emitter's own files is on disk.
///
/// A re-init widens the default agent set under `--yes` with this selection,
/// so existing supported emitters keep receiving scaffold updates. Re-init
/// never removes their files; explicit reset uses [`remove_emitter_files`].
/// Magistr may update what it owns, but it does not undo a decision the
/// project already made.
#[must_use]
pub fn agents_present(dir: &Path) -> Vec<String> {
    all_emitters()
        .into_iter()
        .filter(|emitter| {
            if emitter.id() == "codex" {
                return super::codex_setup::CodexSetup::is_present(dir);
            }
            emitter
                .removable_paths()
                .iter()
                .any(|path| dir.join(path).is_file())
        })
        .map(|emitter| emitter.id().to_owned())
        .collect()
}

#[cfg(test)]
#[path = "emitters.tests.rs"]
mod tests;
