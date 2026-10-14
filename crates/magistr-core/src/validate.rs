//! Quality gate types and project config loading.
//!
//! Contains the pure data types (`Gate`, `GateResult`, `ValidationResult`) and
//! synchronous helpers (`load_project_config`, `gate_command`). Async gate
//! execution stays in the CLI crate (`src/detection/validate.rs`).

use std::path::Path;
use std::time::Duration;

use crate::error::{MagistrError, Result};
use crate::types::{ProjectConfig, QualityGate};

/// A quality gate step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    Check,
    Test,
    Lint,
}

impl Gate {
    pub fn label(&self) -> &'static str {
        match self {
            Gate::Check => "check",
            Gate::Test => "test",
            Gate::Lint => "lint",
        }
    }

    pub fn all() -> [Gate; 3] {
        [Gate::Check, Gate::Test, Gate::Lint]
    }
}

impl std::fmt::Display for Gate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// Result of running a single quality gate.
#[derive(Debug, Clone)]
pub struct GateResult {
    pub gate: Gate,
    pub passed: bool,
    pub duration: Duration,
    pub output: String,
    pub command: String,
}

impl GateResult {
    pub fn status_label(&self) -> &'static str {
        if self.passed { "PASS" } else { "FAIL" }
    }
}

/// Results from running all quality gates.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub results: Vec<GateResult>,
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

/// Get the shell command for a gate from the quality gate config.
pub fn gate_command(qg: &QualityGate, gate: Gate) -> &str {
    match gate {
        Gate::Check => &qg.check,
        Gate::Test => &qg.test,
        Gate::Lint => &qg.lint,
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
