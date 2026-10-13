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

/// Load project config from `.magistr/project.json` in the given directory.
///
/// If the stored stack has `unknown`/`none` values but auto-detection now
/// finds real values (e.g. because `magistr init` ran before source code existed),
/// the config file is silently updated on disk.
pub fn load_project_config(dir: &Path) -> Result<ProjectConfig> {
    let config_path = crate::paths::project_json(dir)?;
    if !config_path.exists() {
        return Err(MagistrError::Config(format!(
            "no .magistr/project.json found in {}",
            dir.display()
        )));
    }
    let contents =
        std::fs::read_to_string(&config_path).map_err(|e| MagistrError::io(&config_path, e))?;
    let mut config: ProjectConfig =
        serde_json::from_str(&contents).map_err(|e| MagistrError::Config(e.to_string()))?;

    // A project that predates `roadmap_backend` keeps its YAML roadmap until
    // migrated; a fresh one gets the default. One rule, in `backend_for`.
    let declared = serde_json::from_str::<serde_json::Value>(&contents)
        .ok()
        .and_then(|v| v.get("roadmap_backend").cloned())
        .and_then(|v| serde_json::from_value(v).ok());
    config.roadmap_backend = crate::roadmap::backend_for_declared(dir, declared);

    // Auto-refresh stale stack detection
    if refresh_stale_stack(dir, &mut config) {
        // Best-effort write — don't fail the load if we can't update
        if let Ok(json) = serde_json::to_string_pretty(&config) {
            let _ = std::fs::write(&config_path, json);
        }
    }

    Ok(config)
}

/// Re-run stack detection and update any `unknown`/`none` fields in `config`.
///
/// Returns `true` if any field was refreshed (caller should persist the change).
/// Only upgrades stale values — never overwrites intentional user choices.
fn refresh_stale_stack(dir: &Path, config: &mut ProjectConfig) -> bool {
    let detected = match crate::detection::detect::detect_stack(dir) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let mut changed = false;

    macro_rules! refresh_field {
        ($stored:expr, $detected:expr) => {
            let det_str = $detected.as_str();
            if ($stored == "unknown" || $stored == "none")
                && det_str != "unknown"
                && det_str != "none"
            {
                $stored = det_str.to_string();
                changed = true;
            }
        };
    }

    refresh_field!(config.stack.language, detected.language);
    refresh_field!(config.stack.framework, detected.framework);
    refresh_field!(config.stack.package_manager, detected.package_manager);
    refresh_field!(config.stack.test_framework, detected.test_framework);
    refresh_field!(config.stack.linter, detected.linter);
    refresh_field!(config.stack.formatter, detected.formatter);

    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_labels() {
        assert_eq!(Gate::Check.label(), "check");
        assert_eq!(Gate::Test.label(), "test");
        assert_eq!(Gate::Lint.label(), "lint");
    }

    #[test]
    fn gate_display() {
        assert_eq!(format!("{}", Gate::Check), "check");
    }

    #[test]
    fn gate_all() {
        let all = Gate::all();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn validation_result_all_passed() {
        let result = ValidationResult {
            results: vec![
                GateResult {
                    gate: Gate::Check,
                    passed: true,
                    duration: Duration::from_secs(1),
                    output: String::new(),
                    command: "cargo check".into(),
                },
                GateResult {
                    gate: Gate::Test,
                    passed: true,
                    duration: Duration::from_secs(2),
                    output: String::new(),
                    command: "cargo test".into(),
                },
            ],
        };
        assert!(result.all_passed());
        assert_eq!(result.passed_count(), 2);
        assert_eq!(result.failed_count(), 0);
        assert_eq!(result.total_duration(), Duration::from_secs(3));
    }

    #[test]
    fn validation_result_with_failure() {
        let result = ValidationResult {
            results: vec![
                GateResult {
                    gate: Gate::Check,
                    passed: true,
                    duration: Duration::from_secs(1),
                    output: String::new(),
                    command: "cargo check".into(),
                },
                GateResult {
                    gate: Gate::Test,
                    passed: false,
                    duration: Duration::from_secs(2),
                    output: "test failed".into(),
                    command: "cargo test".into(),
                },
            ],
        };
        assert!(!result.all_passed());
        assert_eq!(result.passed_count(), 1);
        assert_eq!(result.failed_count(), 1);
    }

    #[test]
    fn gate_result_status_label() {
        let pass = GateResult {
            gate: Gate::Check,
            passed: true,
            duration: Duration::ZERO,
            output: String::new(),
            command: String::new(),
        };
        let fail = GateResult {
            gate: Gate::Check,
            passed: false,
            duration: Duration::ZERO,
            output: String::new(),
            command: String::new(),
        };
        assert_eq!(pass.status_label(), "PASS");
        assert_eq!(fail.status_label(), "FAIL");
    }

    #[test]
    fn gate_command_maps_correctly() {
        let qg = QualityGate {
            check: "cargo check".into(),
            test: "cargo test".into(),
            lint: "cargo clippy".into(),
        };
        assert_eq!(gate_command(&qg, Gate::Check), "cargo check");
        assert_eq!(gate_command(&qg, Gate::Test), "cargo test");
        assert_eq!(gate_command(&qg, Gate::Lint), "cargo clippy");
    }
}
