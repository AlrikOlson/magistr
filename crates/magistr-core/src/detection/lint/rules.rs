//! Lint rules for CLAUDE.md anti-pattern detection.
//!
//! Each rule is a function that takes the file content and returns
//! zero or more `LintDiagnostic` values.

use super::{LintDiagnostic, Severity};

/// Run all lint rules against the given CLAUDE.md content.
pub fn run_all(content: &str) -> Vec<LintDiagnostic> {
    let mut diagnostics = Vec::new();
    diagnostics.extend(check_file_too_long(content));
    diagnostics.extend(check_wall_of_text(content));
    diagnostics.extend(check_vague_rules(content));
    diagnostics.extend(check_contradictions(content));
    diagnostics.extend(check_duplicate_sections(content));
    diagnostics.extend(check_no_quality_gates(content));
    diagnostics.extend(check_no_structure(content));
    diagnostics.extend(check_deprecated_patterns(content));
    diagnostics
}

/// Maximum recommended line count for CLAUDE.md.
const MAX_LINES: usize = 500;

/// Lines over this threshold trigger a warning about approaching the limit.
const WARN_LINES: usize = 300;

/// CLAUDE.md files over 500 lines risk being truncated or ignored.
fn check_file_too_long(content: &str) -> Vec<LintDiagnostic> {
    let line_count = content.lines().count();
    if line_count > MAX_LINES {
        vec![LintDiagnostic {
            rule: "too-long",
            severity: Severity::Error,
            message: format!("CLAUDE.md is {line_count} lines (max recommended: {MAX_LINES})"),
            line: None,
            fix: Some(
                "Split into .claude/rules/ files. Keep CLAUDE.md as a brief overview.".into(),
            ),
        }]
    } else if line_count > WARN_LINES {
        vec![LintDiagnostic {
            rule: "too-long",
            severity: Severity::Warning,
            message: format!(
                "CLAUDE.md is {line_count} lines — approaching the {MAX_LINES}-line limit"
            ),
            line: None,
            fix: Some("Consider moving detailed rules to .claude/rules/ files.".into()),
        }]
    } else {
        vec![]
    }
}

/// Paragraphs over 200 words without structure are hard for Claude to parse.
fn check_wall_of_text(content: &str) -> Vec<LintDiagnostic> {
    let mut diagnostics = Vec::new();
    let mut paragraph_start_line = 1_usize;
    let mut paragraph_words = 0_usize;

    for (i, line) in content.lines().enumerate() {
        let line_num = i + 1;
        let trimmed = line.trim();

        // A blank line or heading ends the current paragraph
        if trimmed.is_empty() || trimmed.starts_with('#') {
            if paragraph_words > 200 {
                diagnostics.push(LintDiagnostic {
                    rule: "wall-of-text",
                    severity: Severity::Warning,
                    message: format!(
                        "Paragraph starting at line {paragraph_start_line} is {paragraph_words} words — hard to parse"
                    ),
                    line: Some(paragraph_start_line),
                    fix: Some(
                        "Break into bullet points, code blocks, or shorter paragraphs."
                            .into(),
                    ),
                });
            }
            paragraph_words = 0;
            paragraph_start_line = line_num + 1;
        } else {
            paragraph_words += trimmed.split_whitespace().count();
        }
    }

    // Check final paragraph
    if paragraph_words > 200 {
        diagnostics.push(LintDiagnostic {
            rule: "wall-of-text",
            severity: Severity::Warning,
            message: format!(
                "Paragraph starting at line {paragraph_start_line} is {paragraph_words} words — hard to parse"
            ),
            line: Some(paragraph_start_line),
            fix: Some(
                "Break into bullet points, code blocks, or shorter paragraphs."
                    .into(),
            ),
        });
    }

    diagnostics
}

/// Vague phrases that provide no actionable guidance.
const VAGUE_PHRASES: &[&str] = &[
    "write good code",
    "be careful",
    "use best practices",
    "follow standards",
    "write clean code",
    "keep it simple",
    "do the right thing",
    "be smart about",
    "use common sense",
    "write quality code",
];

/// Rules that are too vague to be actionable.
fn check_vague_rules(content: &str) -> Vec<LintDiagnostic> {
    let lower = content.to_lowercase();
    let mut diagnostics = Vec::new();

    for (i, line) in lower.lines().enumerate() {
        let line_num = i + 1;
        for phrase in VAGUE_PHRASES {
            if line.contains(phrase) {
                diagnostics.push(LintDiagnostic {
                    rule: "vague-rule",
                    severity: Severity::Warning,
                    message: format!("Vague instruction: \"{phrase}\""),
                    line: Some(line_num),
                    fix: Some(
                        "Replace with a specific, testable rule (e.g., \"Functions must be under 30 lines\")."
                            .into(),
                    ),
                });
            }
        }
    }

    diagnostics
}

/// Contradiction pairs — if both patterns appear, flag a conflict.
const CONTRADICTION_PAIRS: &[(&str, &str)] = &[
    ("always use unwrap", "never use unwrap"),
    ("always use println", "never use println"),
    ("no comments", "comment everything"),
    ("avoid abstractions", "use abstractions"),
    ("no tests", "always write tests"),
    ("no error handling", "handle all errors"),
];

/// Detect contradictory instructions.
fn check_contradictions(content: &str) -> Vec<LintDiagnostic> {
    let lower = content.to_lowercase();
    let mut diagnostics = Vec::new();

    for (a, b) in CONTRADICTION_PAIRS {
        if lower.contains(a) && lower.contains(b) {
            diagnostics.push(LintDiagnostic {
                rule: "contradiction",
                severity: Severity::Error,
                message: format!("Contradictory instructions: \"{a}\" vs \"{b}\""),
                line: None,
                fix: Some("Remove one of the conflicting instructions.".into()),
            });
        }
    }

    diagnostics
}

/// Duplicate section headings cause confusion.
fn check_duplicate_sections(content: &str) -> Vec<LintDiagnostic> {
    let mut headings: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut diagnostics = Vec::new();

    for (i, line) in content.lines().enumerate() {
        let line_num = i + 1;
        let trimmed = line.trim();
        if let Some(heading) = trimmed.strip_prefix("## ") {
            let normalized = heading.trim().to_lowercase();
            if let Some(first_line) = headings.get(&normalized) {
                diagnostics.push(LintDiagnostic {
                    rule: "duplicate-section",
                    severity: Severity::Warning,
                    message: format!(
                        "Duplicate section \"{}\" (first at line {first_line})",
                        heading.trim()
                    ),
                    line: Some(line_num),
                    fix: Some("Merge duplicate sections into one.".into()),
                });
            } else {
                headings.insert(normalized, line_num);
            }
        }
    }

    diagnostics
}

/// Quality gate keywords that indicate test/check/lint commands are defined.
const GATE_KEYWORDS: &[&str] = &[
    "cargo test",
    "cargo clippy",
    "npm test",
    "npx jest",
    "npx vitest",
    "pytest",
    "go test",
    "npm run lint",
    "npx eslint",
    "ruff check",
    "cargo check",
    "npm run build",
    "tsc --noEmit",
];

/// Missing quality gates means Claude won't know how to verify its work.
fn check_no_quality_gates(content: &str) -> Vec<LintDiagnostic> {
    let lower = content.to_lowercase();
    let has_gates = GATE_KEYWORDS.iter().any(|kw| lower.contains(kw));

    if has_gates {
        vec![]
    } else {
        vec![LintDiagnostic {
            rule: "missing-gates",
            severity: Severity::Warning,
            message: "No quality gate commands found (test, lint, check)".into(),
            line: None,
            fix: Some(
                "Add a section with commands like `cargo test`, `npm test`, `pytest`, etc.".into(),
            ),
        }]
    }
}

/// A CLAUDE.md with no headings or bullet points is unstructured.
fn check_no_structure(content: &str) -> Vec<LintDiagnostic> {
    if content.trim().is_empty() {
        return vec![LintDiagnostic {
            rule: "empty-file",
            severity: Severity::Error,
            message: "CLAUDE.md is empty".into(),
            line: None,
            fix: Some("Add project context, conventions, and quality gates.".into()),
        }];
    }

    let has_headings = content.lines().any(|l| l.trim().starts_with('#'));
    let has_bullets = content.lines().any(|l| {
        let t = l.trim();
        t.starts_with("- ") || t.starts_with("* ") || t.starts_with("1.")
    });

    if !has_headings && !has_bullets && content.lines().count() > 10 {
        vec![LintDiagnostic {
            rule: "no-structure",
            severity: Severity::Warning,
            message: "CLAUDE.md has no headings or lists — unstructured text is hard for Claude to follow".into(),
            line: None,
            fix: Some(
                "Add markdown headings (##) and bullet points to organize rules."
                    .into(),
            ),
        }]
    } else {
        vec![]
    }
}

/// Deprecated or outdated patterns.
const DEPRECATED_PATTERNS: &[(&str, &str)] = &[
    (
        "eslintrc",
        "ESLint flat config (eslint.config.js) is now the default — .eslintrc is deprecated",
    ),
    (
        "tslint",
        "TSLint is deprecated — use ESLint with @typescript-eslint",
    ),
    (
        "create-react-app",
        "create-react-app is unmaintained — use Vite or Next.js",
    ),
];

/// Flag references to deprecated tools or patterns.
fn check_deprecated_patterns(content: &str) -> Vec<LintDiagnostic> {
    let lower = content.to_lowercase();
    let mut diagnostics = Vec::new();

    for (i, line) in lower.lines().enumerate() {
        let line_num = i + 1;
        for (pattern, explanation) in DEPRECATED_PATTERNS {
            if line.contains(pattern) {
                diagnostics.push(LintDiagnostic {
                    rule: "deprecated-pattern",
                    severity: Severity::Info,
                    message: explanation.to_string(),
                    line: Some(line_num),
                    fix: None,
                });
            }
        }
    }

    diagnostics
}

#[cfg(test)]
#[path = "rules.tests.rs"]
mod tests;
