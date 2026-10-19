//! System prompt builder for agent sessions.
//!
//! Loads project configuration from `.magistr/project.json` and builds
//! a structured system prompt that gives the agent context about the project.

use std::path::Path;

use crate::types::ProjectConfig;

/// Build a system prompt from the project configuration in the given directory.
///
/// Returns `None` if no `.magistr/project.json` exists or it can't be parsed.
pub fn build_system_prompt(project_dir: &Path) -> Option<String> {
    let config_path = crate::paths::project_json(project_dir).ok()?;
    let content = std::fs::read_to_string(&config_path).ok()?;
    let config: ProjectConfig = serde_json::from_str(&content).ok()?;

    let mut prompt = format_prompt(&config);

    // Dynamically load all .claude/rules/*.md files
    let rules = load_rules(project_dir);
    if !rules.is_empty() {
        prompt.push_str("\n---\n\n# Project Rules\n\n");
        for (name, content) in &rules {
            prompt.push_str(&format!("## {name}\n\n{content}\n\n"));
        }
    }

    // Inject project memory if it exists (.magistr/memory.md)
    if let Some(memory_content) = load_memory(project_dir) {
        prompt.push_str("\n---\n\n");
        prompt.push_str(&memory_content);
        prompt.push('\n');
    }

    // Keep the recording contract current even when project rules predate it.
    // Generated tool guides already carry the same bytes, so include it once.
    let reasoning = magistr_engine::prompt::PromptConfig::REASONING_GUIDANCE;
    if !prompt.contains(reasoning) {
        prompt.push_str("\n---\n\n");
        prompt.push_str(reasoning);
    }

    // Recovery instructions override older fresh-start recipes without editing
    // customized project rules. They apply only to authorized roadmap work.
    let recovery = magistr_engine::prompt::PromptConfig::NEXT_RECOVERY_GUIDANCE;
    if !prompt.contains(recovery) {
        prompt.push_str("\n---\n\nFor authorized roadmap work (`next` or `next-loop`), follow this recovery policy. It does not authorize starting roadmap work for an unrelated request.\n\n");
        prompt.push_str(recovery);
    }

    Some(prompt)
}

/// Load all `.claude/rules/*.md` files, sorted by name.
///
/// Returns `(stem, content)` pairs where stem is the filename without extension.
fn load_rules(project_dir: &Path) -> Vec<(String, String)> {
    let rules_dir = crate::paths::rules_dir(project_dir);
    let Ok(entries) = std::fs::read_dir(&rules_dir) else {
        return Vec::new();
    };

    let mut rules: Vec<(String, String)> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
        .filter_map(|e| {
            let path = e.path();
            let stem = path.file_stem()?.to_string_lossy().to_string();
            let content = std::fs::read_to_string(&path).ok()?;
            Some((stem, content))
        })
        .collect();

    rules.sort_by(|a, b| a.0.cmp(&b.0));
    rules
}

/// Load project memory from `.magistr/memory.md` if it exists.
///
/// Returns the raw markdown content, which is already human-readable
/// and structured with category headings. Returns `None` if the file
/// doesn't exist or is empty.
fn load_memory(project_dir: &Path) -> Option<String> {
    let memory_path = crate::paths::memory(project_dir);
    let content = std::fs::read_to_string(&memory_path).ok()?;
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_string())
}

/// Format a `ProjectConfig` into a system prompt string.
fn format_prompt(config: &ProjectConfig) -> String {
    let mut prompt = String::with_capacity(1024);

    prompt.push_str("# Project Context\n\n");

    // Project info
    prompt.push_str(&format!("**Project:** {}\n", config.project.name));
    if !config.project.description.is_empty() {
        prompt.push_str(&format!(
            "**Description:** {}\n",
            config.project.description
        ));
    }
    if !config.project.project_type.is_empty() {
        prompt.push_str(&format!("**Type:** {}\n", config.project.project_type));
    }
    prompt.push('\n');

    // Stack
    prompt.push_str("## Stack\n\n");
    prompt.push_str(&format!("- Language: {}\n", config.stack.language));
    if config.stack.framework != "none" {
        prompt.push_str(&format!("- Framework: {}\n", config.stack.framework));
    }
    prompt.push_str(&format!(
        "- Package Manager: {}\n",
        config.stack.package_manager
    ));
    prompt.push_str(&format!(
        "- Test Framework: {}\n",
        config.stack.test_framework
    ));
    prompt.push_str(&format!("- Linter: {}\n", config.stack.linter));
    prompt.push_str(&format!("- Formatter: {}\n", config.stack.formatter));
    prompt.push('\n');

    // Quality gates
    prompt.push_str("## Quality Gates\n\n");
    prompt.push_str("Run these commands to validate changes:\n\n");
    prompt.push_str(&format!("- **Check:** `{}`\n", config.quality_gate.check));
    prompt.push_str(&format!("- **Test:** `{}`\n", config.quality_gate.test));
    prompt.push_str(&format!("- **Lint:** `{}`\n", config.quality_gate.lint));
    if !config.custom_gates.is_empty() {
        prompt.push_str("\n**Custom gates:**\n");
        for cg in &config.custom_gates {
            let req = if cg.required { "required" } else { "optional" };
            prompt.push_str(&format!("- **{}** ({req}): `{}`\n", cg.name, cg.command));
        }
    }
    prompt.push('\n');

    // Format
    prompt.push_str("## Formatting\n\n");
    prompt.push_str(&format!("- **Command:** `{}`\n", config.format.command));
    prompt.push_str(&format!(
        "- **Extensions:** {}\n",
        config.format.extensions.join(", ")
    ));
    prompt.push('\n');

    // Spec — absent on a magistr project, where the roadmap lives in
    // the native engines and is read through `roadmap_next`.
    prompt.push_str("## Specification\n\n");
    if let Some(spec) = &config.spec {
        prompt.push_str(&format!("- **Spec file:** `{}`\n", spec.file));
        prompt.push_str("- Update the spec file after implementing changes\n");
    } else {
        prompt.push_str(
            "- The roadmap lives in magistr — read it with `roadmap_next` / \
             `roadmap_status`, never from a file\n",
        );
        prompt.push_str(
            "- Update it with `roadmap_complete_chunk` / `roadmap_add_chunk` after \
             implementing changes\n",
        );
    }

    // Hints — free-form project-specific context
    if !config.hints.is_empty() {
        prompt.push_str("\n## Project Hints\n\n");
        for hint in &config.hints {
            prompt.push_str(&format!("- {hint}\n"));
        }
    }

    // Lifecycle hooks — tell the agent what fires automatically
    if !config.lifecycle_hooks.is_empty() {
        prompt.push_str("\n## Lifecycle Hooks\n\n");
        prompt.push_str(
            "The following tools fire automatically at specific workflow events. \
             **Do not call these tools manually** — they run as part of the lifecycle.\n\n",
        );

        // Group hooks by event, preserving declaration order within each group.
        let mut groups: Vec<(
            &magistr_engine::LifecycleEvent,
            Vec<&magistr_engine::LifecycleHook>,
        )> = Vec::new();
        for hook in &config.lifecycle_hooks {
            if let Some((_event, hooks)) = groups.iter_mut().find(|(e, _)| *e == &hook.event) {
                hooks.push(hook);
            } else {
                groups.push((&hook.event, vec![hook]));
            }
        }

        for (event, hooks) in &groups {
            prompt.push_str(&format!("### `{event}`\n\n"));
            for hook in hooks {
                prompt.push_str(&format!("- `{}`", hook.display_name()));
                if let Some(ref desc) = hook.description {
                    prompt.push_str(&format!(" — {desc}"));
                }
                prompt.push('\n');
            }
            prompt.push('\n');
        }
    }

    // NOTE: Tool guidance (magistr, workflow tools, preferences) is loaded
    // dynamically from .claude/rules/tools.md by build_system_prompt() — not hardcoded here.
    // The single source of truth is scaffold::tool_guide::build_tool_guide().

    prompt
}

#[cfg(test)]
#[path = "system_prompt.tests.rs"]
mod tests;
