//! Stack-based CLAUDE.md improvement suggestions.
//!
//! Given a detected project stack and existing CLAUDE.md content,
//! suggest rules and conventions that are commonly useful for that stack.

use crate::types::{Framework, Language, Linter, Stack, TestFramework};

pub use super::suggestion::Suggestion;

/// Generate improvement suggestions based on the detected stack and existing CLAUDE.md content.
///
/// Checks which common rules for the stack are missing from the existing content
/// and suggests adding them.
pub fn suggest_improvements(stack: &Stack, existing_content: &str) -> Vec<Suggestion> {
    let lower = existing_content.to_lowercase();
    let mut suggestions = Vec::new();

    suggestions.extend(language_suggestions(stack.language, &lower));
    suggestions.extend(framework_suggestions(stack.framework, &lower));
    suggestions.extend(test_suggestions(stack.test_framework, &lower));
    suggestions.extend(linter_suggestions(stack.linter, &lower));
    suggestions.extend(general_suggestions(&lower));

    suggestions
}

/// Language-specific rule suggestions.
fn language_suggestions(lang: Language, existing: &str) -> Vec<Suggestion> {
    match lang {
        Language::Rust => rust_suggestions(existing),
        Language::TypeScript | Language::JavaScript => ts_suggestions(existing),
        Language::Python => python_suggestions(existing),
        Language::Go => go_suggestions(existing),
        _ => vec![],
    }
}

fn rust_suggestions(existing: &str) -> Vec<Suggestion> {
    let mut s = Vec::new();

    if !existing.contains("unwrap") {
        s.push(Suggestion {
            category: "error-handling",
            rule: "No `unwrap()` in library code — use `?` or `.expect(\"reason\")` with context"
                .into(),
            rationale: "unwrap() causes panics with no context. Use ? for propagation or expect() with a message."
                .into(),
        });
    }

    if !existing.contains("clippy") {
        s.push(Suggestion {
            category: "linting",
            rule: "Run `cargo clippy -- -D warnings` before committing".into(),
            rationale: "Clippy catches common Rust mistakes and enforces idiomatic patterns."
                .into(),
        });
    }

    if !existing.contains("unsafe") {
        s.push(Suggestion {
            category: "safety",
            rule: "No `unsafe` without a safety comment and justification".into(),
            rationale: "unsafe blocks need documented safety invariants to maintain soundness."
                .into(),
        });
    }

    if !existing.contains("clone") && !existing.contains("borrow") {
        s.push(Suggestion {
            category: "performance",
            rule: "No unnecessary `.clone()` to satisfy the borrow checker".into(),
            rationale: "Cloning to fix borrow errors often indicates a design issue. Understand ownership first.".into(),
        });
    }

    s
}

fn ts_suggestions(existing: &str) -> Vec<Suggestion> {
    let mut s = Vec::new();

    if !existing.contains("any") {
        s.push(Suggestion {
            category: "type-safety",
            rule: "Avoid `any` — use `unknown` or specific types".into(),
            rationale: "`any` disables TypeScript's type checking, defeating its purpose.".into(),
        });
    }

    if !existing.contains("strict") && !existing.contains("tsconfig") {
        s.push(Suggestion {
            category: "type-safety",
            rule: "Enable strict mode in tsconfig.json".into(),
            rationale: "Strict mode catches null/undefined errors and enforces better types."
                .into(),
        });
    }

    if !existing.contains("console.log") {
        s.push(Suggestion {
            category: "code-quality",
            rule: "Remove `console.log` before committing — use a proper logger".into(),
            rationale: "console.log statements are debug leftovers that clutter output.".into(),
        });
    }

    s
}

fn python_suggestions(existing: &str) -> Vec<Suggestion> {
    let mut s = Vec::new();

    if !existing.contains("type hint")
        && !existing.contains("type annotation")
        && !existing.contains("mypy")
    {
        s.push(Suggestion {
            category: "type-safety",
            rule: "Use type hints on all function signatures".into(),
            rationale:
                "Type hints improve readability and enable static analysis with mypy/pyright."
                    .into(),
        });
    }

    if !existing.contains("bare except")
        && !existing.contains("except:")
        && !existing.contains("exception")
    {
        s.push(Suggestion {
            category: "error-handling",
            rule: "No bare `except:` — always catch specific exceptions".into(),
            rationale: "Bare except catches KeyboardInterrupt and SystemExit, hiding real errors."
                .into(),
        });
    }

    if !existing.contains("f-string") && !existing.contains("format(") {
        s.push(Suggestion {
            category: "code-style",
            rule: "Prefer f-strings over `.format()` or `%` string formatting".into(),
            rationale: "f-strings are more readable and performant in Python 3.6+.".into(),
        });
    }

    s
}

fn go_suggestions(existing: &str) -> Vec<Suggestion> {
    let mut s = Vec::new();

    if !existing.contains("error wrapping")
        && !existing.contains("fmt.errorf")
        && !existing.contains("%w")
    {
        s.push(Suggestion {
            category: "error-handling",
            rule: "Wrap errors with context using `fmt.Errorf(\"doing X: %w\", err)`".into(),
            rationale: "Error wrapping preserves the chain and makes debugging easier.".into(),
        });
    }

    if !existing.contains("golangci") {
        s.push(Suggestion {
            category: "linting",
            rule: "Run `golangci-lint run` before committing".into(),
            rationale: "golangci-lint runs multiple Go linters in one pass.".into(),
        });
    }

    if !existing.contains("goroutine") && !existing.contains("concurrency") {
        s.push(Suggestion {
            category: "concurrency",
            rule:
                "Document goroutine lifecycle — who starts it, who stops it, how errors propagate"
                    .into(),
            rationale: "Undocumented goroutines are a common source of leaks and race conditions."
                .into(),
        });
    }

    s
}

/// Framework-specific suggestions.
fn framework_suggestions(fw: Framework, existing: &str) -> Vec<Suggestion> {
    match fw {
        Framework::Next => next_suggestions(existing),
        Framework::Django => django_suggestions(existing),
        Framework::React => react_suggestions(existing),
        _ => vec![],
    }
}

fn next_suggestions(existing: &str) -> Vec<Suggestion> {
    let mut s = Vec::new();

    if !existing.contains("server component") && !existing.contains("use client") {
        s.push(Suggestion {
            category: "next.js",
            rule:
                "Default to Server Components — only add 'use client' when needed for interactivity"
                    .into(),
            rationale: "Server Components reduce client bundle size and improve performance."
                .into(),
        });
    }

    s
}

fn django_suggestions(existing: &str) -> Vec<Suggestion> {
    let mut s = Vec::new();

    if !existing.contains("migration") {
        s.push(Suggestion {
            category: "django",
            rule: "Always create and review migrations for model changes".into(),
            rationale: "Forgetting migrations causes schema drift between environments.".into(),
        });
    }

    s
}

fn react_suggestions(existing: &str) -> Vec<Suggestion> {
    let mut s = Vec::new();

    if !existing.contains("key") && !existing.contains("key prop") {
        s.push(Suggestion {
            category: "react",
            rule: "Always provide stable `key` props in lists — never use array index".into(),
            rationale: "Index keys cause incorrect re-renders and state bugs when lists change."
                .into(),
        });
    }

    s
}

/// Test framework-specific suggestions.
fn test_suggestions(tf: TestFramework, existing: &str) -> Vec<Suggestion> {
    let mut s = Vec::new();

    if !existing.contains("test") && !existing.contains("tdd") {
        s.push(Suggestion {
            category: "testing",
            rule: format!("Write tests using {} for all new features", tf.as_str()),
            rationale: "Tests prevent regressions and document expected behavior.".into(),
        });
    }

    s
}

/// Linter-specific suggestions.
fn linter_suggestions(linter: Linter, existing: &str) -> Vec<Suggestion> {
    let mut s = Vec::new();

    if linter != Linter::None && !existing.contains(linter.as_str()) {
        s.push(Suggestion {
            category: "linting",
            rule: format!("Run {} before committing", linter.as_str()),
            rationale: "Consistent linting catches style issues and potential bugs.".into(),
        });
    }

    s
}

/// General suggestions that apply to any project.
fn general_suggestions(existing: &str) -> Vec<Suggestion> {
    let mut s = Vec::new();

    if !existing.contains("commit") {
        s.push(Suggestion {
            category: "workflow",
            rule: "Use conventional commits: feat:, fix:, refactor:, test:, docs:, chore:".into(),
            rationale: "Consistent commit messages enable automated changelogs and clear history."
                .into(),
        });
    }

    if !existing.contains("secret") && !existing.contains("env") && !existing.contains(".env") {
        s.push(Suggestion {
            category: "security",
            rule: "Never commit secrets — use environment variables or .env files (gitignored)"
                .into(),
            rationale: "Secrets in code are the #1 security mistake in AI-assisted development."
                .into(),
        });
    }

    s
}

#[cfg(test)]
#[path = "suggest.tests.rs"]
mod tests;
