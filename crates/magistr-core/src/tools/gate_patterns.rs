//! Quality gate command classification — single source of truth.
//!
//! Consolidates gate detection from the workflow tracker and the TUI handler.
//! Both consumers now delegate here instead of maintaining separate pattern lists.

use crate::types::QualityGate;

/// Quality gate command classification patterns.
///
/// All gate command detection in the codebase delegates to this struct.
/// Adding a new language's gate commands means updating these constants.
pub struct GatePatterns;

/// Result of classifying a bash command against gate patterns.
#[derive(Debug, Clone, Copy, Default)]
pub struct GateMatch {
    pub has_check: bool,
    pub has_test: bool,
    pub has_lint: bool,
}

impl GateMatch {
    /// True if any gate pattern matched.
    #[must_use]
    pub fn any(&self) -> bool {
        self.has_check || self.has_test || self.has_lint
    }
}

impl GatePatterns {
    /// Check command patterns.
    pub const CHECK: &[&str] = &["cargo check", "tsc", "go build", "go vet"];

    /// Test command patterns.
    pub const TEST: &[&str] = &[
        "cargo test",
        "cargo nextest",
        "cargo llvm-cov",
        "npm test",
        "npx jest",
        "pytest",
        "python -m pytest",
        "go test",
    ];

    /// Lint command patterns (full commands for `is_quality_gate` matching).
    pub const LINT_COMMANDS: &[&str] = &[
        "cargo clippy",
        "npm run lint",
        "python -m mypy",
        "ruff check",
        "golangci-lint",
    ];

    /// Lint keyword patterns (shorter, for detecting lint in compound commands).
    pub const LINT_KEYWORDS: &[&str] = &["clippy", "lint", "mypy", "ruff", "golangci"];

    /// Classify which quality gates a bash command covers.
    pub fn classify_command(cmd: &str) -> GateMatch {
        GateMatch {
            has_check: Self::CHECK.iter().any(|p| cmd.contains(p)),
            has_test: Self::TEST.iter().any(|p| cmd.contains(p)),
            has_lint: Self::LINT_KEYWORDS.iter().any(|p| cmd.contains(p)),
        }
    }

    /// Returns true if a bash command is a full quality gate (requires lint).
    ///
    /// When a config is available, requires the configured lint command.
    /// Lint alone is a gate signal (agents sometimes run lint separately).
    /// Test alone is NOT — it could be a TDD cycle.
    pub fn is_quality_gate(cmd: &str, gate: &Option<QualityGate>) -> bool {
        if let Some(qg) = gate {
            let has_lint = !qg.lint.is_empty() && cmd.contains(&qg.lint);
            return has_lint;
        }
        // Fallback: must contain a full lint command
        Self::LINT_COMMANDS.iter().any(|p| cmd.contains(p))
    }

    /// Returns true if the command runs ONLY tests (TDD cycle), not the full gate.
    pub fn is_test_only(cmd: &str) -> bool {
        let has_test = Self::TEST.iter().any(|p| cmd.contains(p));
        let has_lint = Self::LINT_KEYWORDS.iter().any(|p| cmd.contains(p));
        has_test && !has_lint
    }
}

#[cfg(test)]
#[path = "gate_patterns.tests.rs"]
mod tests;
