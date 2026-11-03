//! Check runner for the orchestration layer.
//!
//! Wraps `validate.rs` check execution with:
//! - Structured `CheckRunner` that reads config from `.magistr/project.json`
//! - Parallel check execution (check + lint concurrent, then test)
//! - Structured agent feedback generation on failure

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::types::StandardChecks;
use crate::validate::{self, StandardCheck, StandardCheckResult, ValidationResult};
use crate::workflow::check_cache::CheckCache;

// Re-export CheckState from engine.
pub use magistr_engine::check::CheckState;

// ── Check runner ─────────────────────────────────────────────────────

/// Orchestration-layer check runner.
///
/// Reads check commands from project config, executes them with caching and
/// parallelism, and generates structured feedback for agent steering.
#[derive(Debug, Clone)]
pub struct CheckRunner {
    /// Standard check commands from `.magistr/project.json`.
    checks: StandardChecks,
    /// Project root directory.
    project_dir: PathBuf,
    /// Hash-based result cache.
    cache: CheckCache,
}

impl CheckRunner {
    /// Create a new runner for the given project.
    pub fn new(checks: StandardChecks, project_dir: impl Into<PathBuf>) -> Self {
        Self {
            checks,
            project_dir: project_dir.into(),
            cache: CheckCache::new(),
        }
    }

    /// Run all checks, using cache if valid.
    ///
    /// Returns cached results if no source files have changed since the last
    /// run. Otherwise, runs checks with parallel execution.
    pub async fn run_all(&mut self) -> crate::error::Result<ValidationResult> {
        // Check cache first
        if let Some(cached) = self.cache.get(&self.project_dir) {
            return Ok(cached.clone());
        }

        let result = self.run_parallel().await?;
        self.cache.store(&self.project_dir, result.clone());
        Ok(result)
    }

    /// Run checks with dependency-aware parallelism.
    ///
    /// Runs check + lint concurrently (no dependency between them),
    /// then runs test (which depends on check passing).
    pub async fn run_parallel(&self) -> crate::error::Result<ValidationResult> {
        // Phase 1: check + lint in parallel
        let check_cmd = self.checks.check.clone();
        let lint_cmd = self.checks.lint.clone();
        let dir1 = self.project_dir.clone();
        let dir2 = self.project_dir.clone();

        let (check_result, lint_result) = tokio::join!(
            async { validate::run_single_check(StandardCheck::Check, &check_cmd, &dir1).await },
            async { validate::run_single_check(StandardCheck::Lint, &lint_cmd, &dir2).await },
        );

        // Phase 2: test (only if check passed — test depends on compilation)
        let test_result = if check_result.passed {
            let test_cmd = self.checks.test.clone();
            validate::run_single_check(StandardCheck::Test, &test_cmd, &self.project_dir).await
        } else {
            // Skip test if check failed — it would fail anyway
            StandardCheckResult {
                check: StandardCheck::Test,
                passed: false,
                duration: Duration::ZERO,
                output: "skipped (check failed)".into(),
                command: self.checks.test.clone(),
            }
        };

        Ok(ValidationResult {
            results: vec![check_result, lint_result, test_result],
        })
    }

    /// Generate structured feedback for the agent when checks fail.
    ///
    /// Returns `None` if all checks passed. Returns a structured message
    /// listing each failed check with its output, suitable for prompt steering.
    #[must_use]
    pub fn feedback_for_agent(result: &ValidationResult) -> Option<String> {
        if result.all_passed() {
            return None;
        }

        let mut feedback =
            String::from("Check failures detected. Fix these before committing:\n\n");

        for check_result in &result.results {
            if !check_result.passed {
                feedback.push_str(&format!(
                    "❌ Check `{}` failed (command: `{}`):\n```\n{}\n```\n\n",
                    check_result.check.label(),
                    check_result.command,
                    truncate_output(&check_result.output, 500),
                ));
            }
        }

        feedback.push_str("Fix the failing checks and run them again before attempting to commit.");

        Some(feedback)
    }

    /// Invalidate the cache (e.g., after file changes detected).
    pub fn invalidate_cache(&mut self) {
        self.cache.invalidate();
    }

    /// Whether the cache currently has valid results.
    pub fn cache_valid(&self) -> bool {
        self.cache.is_valid(&self.project_dir)
    }

    /// Get the last cached result (regardless of validity).
    pub fn last_result(&self) -> Option<&ValidationResult> {
        self.cache.last_result.as_ref()
    }

    /// Get the check config.
    pub fn checks(&self) -> &StandardChecks {
        &self.checks
    }

    /// Get the project directory.
    pub fn project_dir(&self) -> &Path {
        &self.project_dir
    }
}

/// Truncate long output to a maximum number of characters.
fn truncate_output(output: &str, max_chars: usize) -> &str {
    if output.len() <= max_chars {
        output
    } else {
        // Find a safe UTF-8 boundary
        let mut end = max_chars;
        while end > 0 && !output.is_char_boundary(end) {
            end -= 1;
        }
        &output[..end]
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "check_runner.tests.rs"]
mod tests;
