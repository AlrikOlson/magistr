//! Archetype profile generator — produce archetype-specialized `SpecializedRole`
//! instances from `Archetype` definitions.
//!
//! For each archetype in a polyglot project, auto-generates an agent system prompt
//! that includes the archetype's conventions, file patterns, naming rules,
//! framework-specific patterns, and anti-patterns.

use crate::role::{RoleConstraints, SpecializedRole};
use crate::types::Archetype;

/// Generate an archetype-specialized coder role for the given archetype.
///
/// The produced role carries the archetype's conventions in its system prompt,
/// uses the archetype's model preference (falling back to the base coder model),
/// and records the archetype ID for downstream tool policy enforcement.
#[must_use]
pub fn archetype_coder(archetype: &Archetype) -> SpecializedRole {
    let conventions_section = build_conventions_section(archetype);

    let system_prompt = format!(
        "You are a coding specialist for the **{name}** ({lang}{fw}) sub-project.\n\
         Root directory: `{root}`\n\n\
         ## Archetype Conventions\n\n\
         {conventions}\n\n\
         ## Instructions\n\n\
         Follow TDD: write a failing test first, make it pass, then refactor.\n\
         Only modify files under `{root}`. Follow the conventions above strictly.\n\
         Navigate and edit with the workbench-mcp tools (search, map, read, edit, \
         files, run, script, undo); follow .claude/skills/workbench-routing/SKILL.md.\n\n\
         Task: {{task}}",
        name = archetype.name,
        lang = archetype.language,
        fw = archetype
            .framework
            .as_deref()
            .map(|f| format!("/{f}"))
            .unwrap_or_default(),
        root = archetype.root.display(),
        conventions = conventions_section,
    );

    SpecializedRole {
        role_id: format!("coder-{}", archetype.id),
        display_name: format!("Coder ({})", archetype.name),
        system_prompt_template: system_prompt,
        allowed_tools: vec![],
        denied_tools: vec![],
        model_preference: archetype
            .model_preference
            .clone()
            .or_else(|| Some("claude-opus-4-6".into())),
        model_fallback: vec!["claude-sonnet-4-6".into()],
        constraints: RoleConstraints {
            read_only: false,
            max_iterations: 0,
        },
        icon: '\u{1f4bb}', // 💻
        color_index: 1,
        archetype_id: Some(archetype.id.clone()),
    }
}

/// Generate an archetype-specialized tester role.
#[must_use]
pub fn archetype_tester(archetype: &Archetype) -> SpecializedRole {
    let test_section = build_test_section(archetype);

    let system_prompt = format!(
        "You are a test specialist for the **{name}** ({lang}{fw}) sub-project.\n\
         Root directory: `{root}`\n\n\
         ## Testing Conventions\n\n\
         {test_section}\n\n\
         ## Instructions\n\n\
         Focus on writing comprehensive tests. Test error paths, not just happy paths.\n\
         Only write tests for files under `{root}`.\n\
         Navigate and edit with the workbench-mcp tools (search, read, edit, run); \
         follow .claude/skills/workbench-routing/SKILL.md.\n\n\
         Task: {{task}}",
        name = archetype.name,
        lang = archetype.language,
        fw = archetype
            .framework
            .as_deref()
            .map(|f| format!("/{f}"))
            .unwrap_or_default(),
        root = archetype.root.display(),
    );

    SpecializedRole {
        role_id: format!("tester-{}", archetype.id),
        display_name: format!("Tester ({})", archetype.name),
        system_prompt_template: system_prompt,
        allowed_tools: vec![],
        denied_tools: vec![],
        model_preference: archetype
            .model_preference
            .clone()
            .or_else(|| Some("claude-sonnet-4-6".into())),
        model_fallback: vec!["claude-opus-4-6".into()],
        constraints: RoleConstraints {
            read_only: false,
            max_iterations: 0,
        },
        icon: '\u{1f9ea}', // 🧪
        color_index: 3,
        archetype_id: Some(archetype.id.clone()),
    }
}

/// Generate all archetype-specialized roles for a set of archetypes.
///
/// For each archetype, produces a coder and a tester role. Callers can filter
/// or extend the returned set as needed.
#[must_use]
pub fn roles_for_archetypes(archetypes: &[Archetype]) -> Vec<SpecializedRole> {
    let mut roles = Vec::with_capacity(archetypes.len() * 2);
    for arch in archetypes {
        roles.push(archetype_coder(arch));
        roles.push(archetype_tester(arch));
    }
    roles
}

/// Build the conventions section for an archetype's system prompt.
fn build_conventions_section(archetype: &Archetype) -> String {
    let mut sections = Vec::new();

    if let Some(ref naming) = archetype.naming_convention {
        sections.push(format!("- **Naming convention:** {naming}"));
    }
    if let Some(ref module_structure) = archetype.module_structure {
        sections.push(format!("- **Module structure:** {module_structure}"));
    }
    if !archetype.source_patterns.is_empty() {
        sections.push(format!(
            "- **Source patterns:** {}",
            archetype.source_patterns.join(", ")
        ));
    }
    if !archetype.test_patterns.is_empty() {
        sections.push(format!(
            "- **Test patterns:** {}",
            archetype.test_patterns.join(", ")
        ));
    }
    if !archetype.config_patterns.is_empty() {
        sections.push(format!(
            "- **Config files:** {}",
            archetype.config_patterns.join(", ")
        ));
    }

    // Add language-specific anti-patterns.
    let anti_patterns = language_anti_patterns(&archetype.language);
    if !anti_patterns.is_empty() {
        sections.push(format!("- **Anti-patterns to avoid:**\n{anti_patterns}"));
    }

    // Add framework-specific patterns.
    if let Some(ref fw) = archetype.framework {
        let fw_patterns = framework_patterns(fw);
        if !fw_patterns.is_empty() {
            sections.push(format!("- **Framework patterns ({fw}):**\n{fw_patterns}"));
        }
    }

    if sections.is_empty() {
        format!("Language: {}", archetype.language)
    } else {
        sections.join("\n")
    }
}

/// Build the testing conventions section for an archetype.
fn build_test_section(archetype: &Archetype) -> String {
    let mut lines = Vec::new();

    lines.push(format!("- **Language:** {}", archetype.language));

    if !archetype.test_patterns.is_empty() {
        lines.push(format!(
            "- **Test file patterns:** {}",
            archetype.test_patterns.join(", ")
        ));
    }

    let test_cmd = &archetype.quality_gate.test;
    if !test_cmd.is_empty() {
        lines.push(format!("- **Test command:** `{test_cmd}`"));
    }

    let test_style = match archetype.language.as_str() {
        "rust" => {
            "Unit tests in `{name}.tests.rs` alongside the module. Use `assert_eq!`, `assert_ne!`, `assert!`."
        }
        "typescript" | "javascript" => {
            "Use the project's test framework (jest/vitest/mocha). Write `.test.ts` files."
        }
        "python" => "Use pytest. Write `test_*.py` files or `*_test.py` files.",
        "go" => {
            "Write `*_test.go` files alongside the source. Use `testing.T` and table-driven tests."
        }
        "csharp" => "Use xUnit/NUnit/MSTest. Write test classes in a `*.Tests` project.",
        "java" | "kotlin" => "Use JUnit 5. Write test classes in `src/test/`.",
        "ruby" => "Use RSpec or Minitest. Write `*_spec.rb` or `*_test.rb` files.",
        "php" => "Use PHPUnit. Write test classes in `tests/`.",
        "elixir" => "Use ExUnit. Write `*_test.exs` files in `test/`.",
        _ => "Follow the project's testing conventions.",
    };
    lines.push(format!("- **Testing style:** {test_style}"));

    lines.join("\n")
}

/// Language-specific anti-patterns injected into archetype agent prompts.
fn language_anti_patterns(language: &str) -> String {
    match language {
        "rust" => [
            "  - No `unwrap()` in library code — use `?` or `expect(\"context\")`",
            "  - No `clone()` to silence the borrow checker",
            "  - No `unsafe` without `// SAFETY:` comment",
            "  - No wildcard imports except in test modules",
        ]
        .join("\n"),
        "typescript" | "javascript" => [
            "  - No `any` type — use proper types or `unknown`",
            "  - No `var` — use `const` or `let`",
            "  - No implicit returns in multi-line functions",
        ]
        .join("\n"),
        "python" => [
            "  - No mutable default arguments",
            "  - No bare `except:` — catch specific exceptions",
            "  - No `import *` — use explicit imports",
        ]
        .join("\n"),
        "go" => [
            "  - No ignored errors — always check `err`",
            "  - No `panic()` in library code",
            "  - No global mutable state",
        ]
        .join("\n"),
        "csharp" => [
            "  - No `async void` except in event handlers",
            "  - No `dynamic` without justification",
            "  - No string concatenation in loops — use `StringBuilder`",
        ]
        .join("\n"),
        _ => String::new(),
    }
}

/// Framework-specific patterns injected into archetype agent prompts.
fn framework_patterns(framework: &str) -> String {
    match framework {
        "nextjs" => [
            "  - Use App Router (`app/` directory) patterns",
            "  - Server Components by default, `'use client'` only when needed",
            "  - Use `next/image` for images, `next/link` for navigation",
        ]
        .join("\n"),
        "react" | "react-native" => [
            "  - Functional components with hooks",
            "  - No class components unless extending error boundaries",
            "  - Use `useMemo`/`useCallback` judiciously",
        ]
        .join("\n"),
        "django" => [
            "  - Follow Django's MTV (Model-Template-View) pattern",
            "  - Use class-based views for complex views",
            "  - Use Django ORM — no raw SQL unless performance-critical",
        ]
        .join("\n"),
        "axum" | "actix-web" => [
            "  - Use extractors for request parsing",
            "  - Return proper error types that implement `IntoResponse`",
            "  - Use `tower` middleware for cross-cutting concerns",
        ]
        .join("\n"),
        "spring" | "spring-boot" => [
            "  - Use constructor injection — no field injection",
            "  - Use `@Transactional` at the service layer",
            "  - Follow Spring's convention-over-configuration patterns",
        ]
        .join("\n"),
        _ => String::new(),
    }
}

#[cfg(test)]
#[path = "archetype_profile.tests.rs"]
mod tests;
