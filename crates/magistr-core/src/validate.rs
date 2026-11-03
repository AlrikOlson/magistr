//! Check types and project config loading.
//!
//! Contains the pure data types (`StandardCheck`, `StandardCheckResult`, `ValidationResult`) and
//! synchronous helpers (`load_project_config`, `standard_check_command`). Async check
//! execution stays in the CLI crate (`src/detection/validate.rs`).

use std::path::Path;
use std::time::Duration;

use crate::error::{MagistrError, Result};
use crate::types::{ProjectConfig, StandardChecks};

/// A check step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StandardCheck {
    Check,
    Test,
    Lint,
}

impl StandardCheck {
    pub fn label(&self) -> &'static str {
        match self {
            StandardCheck::Check => "check",
            StandardCheck::Test => "test",
            StandardCheck::Lint => "lint",
        }
    }

    pub fn all() -> [StandardCheck; 3] {
        [
            StandardCheck::Check,
            StandardCheck::Test,
            StandardCheck::Lint,
        ]
    }
}

impl std::fmt::Display for StandardCheck {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// Result of running a single check.
#[derive(Debug, Clone)]
pub struct StandardCheckResult {
    pub check: StandardCheck,
    pub passed: bool,
    pub duration: Duration,
    pub output: String,
    pub command: String,
}

impl StandardCheckResult {
    pub fn status_label(&self) -> &'static str {
        if self.passed { "PASS" } else { "FAIL" }
    }
}

/// Results from running all checks.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub results: Vec<StandardCheckResult>,
}

impl ValidationResult {
    pub fn all_passed(&self) -> bool {
        self.results.iter().all(|r| r.passed)
    }

    pub fn passed_count(&self) -> usize {
        self.results.iter().filter(|r| r.passed).count()
    }

    pub fn failed_count(&self) -> usize {
        self.results.iter().filter(|r| !r.passed).count()
    }

    pub fn total_duration(&self) -> Duration {
        self.results.iter().map(|r| r.duration).sum()
    }
}

/// Get the shell command for a check from the check config.
pub fn standard_check_command(qg: &StandardChecks, check: StandardCheck) -> &str {
    match check {
        StandardCheck::Check => &qg.check,
        StandardCheck::Test => &qg.test,
        StandardCheck::Lint => &qg.lint,
    }
}

/// Load the current shared project settings without changing them.
/// Missing, unreadable and invalid settings return an error.
pub fn load_project_config(dir: &Path) -> Result<ProjectConfig> {
    let path = crate::paths::project_json(dir)?;
    crate::config::read_project_settings(&path)?.ok_or_else(|| {
        MagistrError::Config(format!(
            "no .magistr/project.json found at {}",
            path.display()
        ))
    })
}

#[cfg(test)]
#[path = "validate.tests.rs"]
mod tests;
