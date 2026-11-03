//! Check plugin trait — contract for custom check providers.
//!
//! Executable plugins of kind `Check` implement this trait via the
//! JSON-line protocol. The trait defines the single operation that magistr needs
//! from any check: run a check and return findings with a pass/fail
//! outcome.

use std::future::Future;
use std::pin::Pin;
use std::time::{Duration, Instant};

use super::types::PluginError;

/// Severity of a check finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckSeverity {
    /// Blocks the check from passing.
    Error,
    /// Non-blocking but should be addressed.
    Warning,
    /// Informational only.
    Info,
}

impl CheckSeverity {
    /// Parse from a string (case-insensitive).
    #[must_use]
    pub fn from_str_lossy(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "error" | "err" | "critical" | "blocker" => Self::Error,
            "warning" | "warn" | "major" | "minor" => Self::Warning,
            _ => Self::Info,
        }
    }

    /// Label for display.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
        }
    }
}

impl std::fmt::Display for CheckSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// A single finding reported by a check plugin.
#[derive(Debug, Clone, PartialEq)]
pub struct CheckFinding {
    /// File path where the finding was detected (if applicable).
    pub file: Option<String>,
    /// Line number in the file (if applicable).
    pub line: Option<u32>,
    /// Human-readable finding message.
    pub message: String,
    /// Severity of the finding.
    pub severity: CheckSeverity,
    /// Rule identifier (e.g., "S1234" for SonarQube, "MIT" for license checks).
    pub rule_id: Option<String>,
}

/// Configuration for running a check.
#[derive(Debug, Clone)]
pub struct CheckPluginConfig {
    /// Check name to run (e.g., "sonarqube", "license").
    pub check_name: String,
    /// Project directory to check.
    pub project_dir: String,
    /// Optional check-specific configuration.
    pub config: Option<serde_json::Value>,
}

/// Outcome of running a check.
#[derive(Debug, Clone, PartialEq)]
pub struct PluginCheckOutcome {
    /// Whether the check passed overall.
    pub passed: bool,
    /// Name of the check that was run.
    pub check_name: String,
    /// Individual findings discovered during the check.
    pub findings: Vec<CheckFinding>,
    /// Human-readable summary of the check result.
    pub summary: String,
    /// How long the check took to run.
    pub duration: Duration,
}

impl PluginCheckOutcome {
    /// Count findings by severity.
    #[must_use]
    pub fn count_by_severity(&self, severity: CheckSeverity) -> usize {
        self.findings
            .iter()
            .filter(|f| f.severity == severity)
            .count()
    }

    /// Count error-severity findings.
    #[must_use]
    pub fn error_count(&self) -> usize {
        self.count_by_severity(CheckSeverity::Error)
    }

    /// Count warning-severity findings.
    #[must_use]
    pub fn warning_count(&self) -> usize {
        self.count_by_severity(CheckSeverity::Warning)
    }
}

/// Trait for check plugins.
///
/// Check plugins provide custom checks (SonarQube, license compliance,
/// security scanners, etc.) that run alongside the standard check/test/lint checks.
///
/// Implementations communicate with plugin subprocesses via the JSON-line
/// protocol defined in [`super::types::PluginRequest`] and
/// [`super::types::PluginResponse`].
pub trait CheckPlugin: Send {
    /// Run the check.
    ///
    /// Returns a [`PluginCheckOutcome`] with findings and a pass/fail verdict.
    /// Findings may be streamed incrementally by the subprocess, but the
    /// trait collects them into a single outcome.
    fn run_check(
        &mut self,
        config: CheckPluginConfig,
    ) -> Pin<Box<dyn Future<Output = Result<PluginCheckOutcome, PluginError>> + Send + '_>>;

    /// Get the plugin name.
    fn name(&self) -> &str;

    /// Get the check names this plugin can run.
    fn supported_checks(&self) -> &[String];
}

/// A check plugin implementation backed by a [`PluginInstance`].
///
/// Translates trait method calls into JSON-line protocol requests and
/// parses the responses. Mirrors the
/// [`super::workflow_step_trait::SubprocessWorkflowStep`] pattern.
pub struct SubprocessCheck {
    /// The activated plugin instance.
    instance: super::lifecycle::PluginInstance,
    /// Check names reported by the plugin during init (from capabilities).
    supported_checks: Vec<String>,
}

impl SubprocessCheck {
    /// Create a new subprocess check from an activated plugin instance.
    ///
    /// The instance must be in the `Active` state. Supported check names
    /// are extracted from capabilities prefixed with `check:`.
    pub fn new(instance: super::lifecycle::PluginInstance) -> Result<Self, PluginError> {
        if instance.state() != super::lifecycle::PluginState::Active {
            return Err(PluginError::LifecycleViolation(format!(
                "check plugin '{}' must be active (current state: {})",
                instance.manifest().name,
                instance.state()
            )));
        }

        let supported_checks = instance
            .capabilities()
            .iter()
            .filter_map(|cap| cap.strip_prefix("check:").map(String::from))
            .collect();

        Ok(Self {
            instance,
            supported_checks,
        })
    }

    /// Get the underlying plugin instance.
    #[must_use]
    pub fn instance(&self) -> &super::lifecycle::PluginInstance {
        &self.instance
    }
}

impl CheckPlugin for SubprocessCheck {
    fn run_check(
        &mut self,
        config: CheckPluginConfig,
    ) -> Pin<Box<dyn Future<Output = Result<PluginCheckOutcome, PluginError>> + Send + '_>> {
        Box::pin(async move {
            let start = Instant::now();
            let id = self.instance.process_mut().next_id();
            let request = super::types::PluginRequest::CheckRun {
                id,
                check_name: config.check_name.clone(),
                project_dir: config.project_dir,
                config: config.config,
            };

            self.instance.process_mut().send(&request)?;

            let mut findings = Vec::new();

            // Read streaming findings until we get a CheckDone or Error.
            while let Some(result) = self.instance.process_mut().recv().await {
                match result? {
                    super::types::PluginResponse::CheckFinding {
                        id: resp_id,
                        file,
                        line,
                        message,
                        severity,
                        rule_id,
                    } if resp_id == id => {
                        findings.push(CheckFinding {
                            file,
                            line,
                            message,
                            severity: CheckSeverity::from_str_lossy(&severity),
                            rule_id,
                        });
                    }
                    super::types::PluginResponse::CheckDone {
                        id: resp_id,
                        passed,
                        summary,
                    } if resp_id == id => {
                        return Ok(PluginCheckOutcome {
                            passed,
                            check_name: config.check_name,
                            findings,
                            summary,
                            duration: start.elapsed(),
                        });
                    }
                    super::types::PluginResponse::Error {
                        id: resp_id,
                        message,
                    } if resp_id == id => {
                        return Err(PluginError::PluginError {
                            id: resp_id,
                            message,
                        });
                    }
                    _ => continue, // Non-matching response — skip
                }
            }

            Err(PluginError::ProcessExited(format!(
                "check plugin '{}' exited before completing check '{}'",
                self.instance.manifest().name,
                config.check_name
            )))
        })
    }

    fn name(&self) -> &str {
        &self.instance.manifest().name
    }

    fn supported_checks(&self) -> &[String] {
        &self.supported_checks
    }
}

#[cfg(test)]
#[path = "check_trait.tests.rs"]
mod tests;
