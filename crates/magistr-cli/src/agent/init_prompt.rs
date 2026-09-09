//! System prompt for agent-powered `magistr init --interactive`.

use crate::types::Stack;

/// Build a system prompt for agent-powered `magistr init --interactive`.
///
/// Provides the detected stack as context and instructs the agent to
/// interview the user about their project, then generate config files.
pub fn build_init_prompt(stack: &Stack, project_dir: &str) -> String {
    let stack_json = serde_json::to_string_pretty(stack).unwrap_or_else(|_| format!("{stack:?}"));

    format!(
        r#"# Interactive Project Setup

You are helping set up an AI-assisted development workflow for a project. Your job is to interview the user about their project, then generate the configuration files.

## Detected Stack

The following stack was auto-detected from the project files in `{project_dir}`:

```json
{stack_json}
```

## Your Task

1. **Greet the user** and confirm the detected stack looks right. Ask if anything needs correction.
2. **Ask about the project:**
   - What does this project do? (1-2 sentence description)
   - What type of project is it? (cli, web-app, api, library, monorepo, or other)
   - Any specific conventions or preferences for code quality, testing, or formatting?
3. **Generate the configuration** by writing these files:
   - `.magistr/project.json` — project metadata, stack config, quality gates, formatting, testing config
   - `.claude/settings.json` — allowed tools list for the agent
   - `.claude/rules/workflow.md` — plan-execute-verify workflow rules
   - `.claude/rules/conventions.md` — language-specific coding conventions
   - Any additional rules files appropriate for the detected stack

Keep the interview concise — 2-3 questions max. Don't over-ask. Use the detected stack to fill in sensible defaults and only ask about things that need human input.

## Configuration Format

The `.magistr/project.json` should follow this schema:

```json
{{{{
  "project": {{{{
    "name": "<project_name>",
    "description": "<from interview>",
    "type": "<from interview>"
  }}}},
  "stack": {{{{
    "language": "<detected>",
    "framework": "<detected>",
    "package_manager": "<detected>",
    "test_framework": "<detected>",
    "linter": "<detected>",
    "formatter": "<detected>"
  }}}},
  "quality_gate": {{{{
    "check": "<language-appropriate check command>",
    "test": "<language-appropriate test command>",
    "lint": "<language-appropriate lint command>"
  }}}},
  "format": {{{{
    "extensions": ["<file extensions>"],
    "command": "<format command>"
  }}}},
  "spec": {{{{
    "file": "ROADMAP.md",
    "unchecked_pattern": "^\\s*- \\[ \\]"
  }}}},
  "testing": {{{{
    "companion_pattern": "<test file pattern>",
    "test_dir": "<test directory>"
  }}}},
  "reference_repos": [],
  "state_prefix": "<project-name-slug>"
}}}}
```

Use the detected stack values to fill in appropriate quality gate commands, format settings, and testing config for the language/framework combination."#
    )
}
