//! Quality gate plugin trait — contract for custom quality gate providers.
//!
//! Executable plugins of kind `QualityGate` implement this trait via the
//! JSON-line protocol. The trait defines the single operation that magistr needs
//! from any quality gate: run a check and return findings with a pass/fail
//! outcome.

use std::future::Future;
use std::pin::Pin;
use std::time::{Duration, Instant};

use super::types::PluginError;

/// Severity of a quality gate finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateSeverity {
    /// Blocks the gate from passing.
    Error,
    /// Non-blocking but should be addressed.
    Warning,
    /// Informational only.
    Info,
}

impl GateSeverity {
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

impl std::fmt::Display for GateSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// A single finding reported by a quality gate plugin.
#[derive(Debug, Clone, PartialEq)]
pub struct GateFinding {
    /// File path where the finding was detected (if applicable).
    pub file: Option<String>,
    /// Line number in the file (if applicable).
    pub line: Option<u32>,
    /// Human-readable finding message.
    pub message: String,
    /// Severity of the finding.
    pub severity: GateSeverity,
    /// Rule identifier (e.g., "S1234" for SonarQube, "MIT" for license checks).
    pub rule_id: Option<String>,
}

/// Configuration for running a quality gate.
#[derive(Debug, Clone)]
pub struct GateConfig {
    /// Gate name to run (e.g., "sonarqube", "license").
    pub gate_name: String,
    /// Project directory to check.
    pub project_dir: String,
    /// Optional gate-specific configuration.
    pub config: Option<serde_json::Value>,
}

/// Outcome of running a quality gate.
#[derive(Debug, Clone, PartialEq)]
pub struct GateOutcome {
    /// Whether the gate passed overall.
    pub passed: bool,
    /// Name of the gate that was run.
    pub gate_name: String,
    /// Individual findings discovered during the check.
    pub findings: Vec<GateFinding>,
    /// Human-readable summary of the gate result.
    pub summary: String,
    /// How long the gate took to run.
    pub duration: Duration,
}

impl GateOutcome {
    /// Count findings by severity.
    #[must_use]
    pub fn count_by_severity(&self, severity: GateSeverity) -> usize {
        self.findings
            .iter()
            .filter(|f| f.severity == severity)
            .count()
    }

    /// Count error-severity findings.
    #[must_use]
    pub fn error_count(&self) -> usize {
        self.count_by_severity(GateSeverity::Error)
    }

    /// Count warning-severity findings.
    #[must_use]
    pub fn warning_count(&self) -> usize {
        self.count_by_severity(GateSeverity::Warning)
    }
}

/// Trait for quality gate plugins.
///
/// Quality gate plugins provide custom checks (SonarQube, license compliance,
/// security scanners, etc.) that run alongside the standard check/test/lint gates.
///
/// Implementations communicate with plugin subprocesses via the JSON-line
/// protocol defined in [`super::types::PluginRequest`] and
/// [`super::types::PluginResponse`].
pub trait GatePlugin: Send {
    /// Run the quality gate check.
    ///
    /// Returns a [`GateOutcome`] with findings and a pass/fail verdict.
    /// Findings may be streamed incrementally by the subprocess, but the
    /// trait collects them into a single outcome.
    fn run_gate(
        &mut self,
        config: GateConfig,
    ) -> Pin<Box<dyn Future<Output = Result<GateOutcome, PluginError>> + Send + '_>>;

    /// Get the plugin name.
    fn name(&self) -> &str;

    /// Get the gate names this plugin can run.
    fn supported_gates(&self) -> &[String];
}

/// A quality gate plugin implementation backed by a [`PluginInstance`].
///
/// Translates trait method calls into JSON-line protocol requests and
/// parses the responses. Mirrors the
/// [`super::workflow_step_trait::SubprocessWorkflowStep`] pattern.
pub struct SubprocessGate {
    /// The activated plugin instance.
    instance: super::lifecycle::PluginInstance,
    /// Gate names reported by the plugin during init (from capabilities).
    supported_gates: Vec<String>,
}

impl SubprocessGate {
    /// Create a new subprocess gate from an activated plugin instance.
    ///
    /// The instance must be in the `Active` state. Supported gate names
    /// are extracted from capabilities prefixed with `gate:`.
    pub fn new(instance: super::lifecycle::PluginInstance) -> Result<Self, PluginError> {
        if instance.state() != super::lifecycle::PluginState::Active {
            return Err(PluginError::LifecycleViolation(format!(
                "gate plugin '{}' must be active (current state: {})",
                instance.manifest().name,
                instance.state()
            )));
        }

        let supported_gates = instance
            .capabilities()
            .iter()
            .filter_map(|cap| cap.strip_prefix("gate:").map(String::from))
            .collect();

        Ok(Self {
            instance,
            supported_gates,
        })
    }

    /// Get the underlying plugin instance.
    #[must_use]
    pub fn instance(&self) -> &super::lifecycle::PluginInstance {
        &self.instance
    }
}

impl GatePlugin for SubprocessGate {
    fn run_gate(
        &mut self,
        config: GateConfig,
    ) -> Pin<Box<dyn Future<Output = Result<GateOutcome, PluginError>> + Send + '_>> {
        Box::pin(async move {
            let start = Instant::now();
            let id = self.instance.process_mut().next_id();
            let request = super::types::PluginRequest::GateRun {
                id,
                gate_name: config.gate_name.clone(),
                project_dir: config.project_dir,
                config: config.config,
            };

            self.instance.process_mut().send(&request)?;

            let mut findings = Vec::new();

            // Read streaming findings until we get a GateDone or Error.
            while let Some(result) = self.instance.process_mut().recv().await {
                match result? {
                    super::types::PluginResponse::GateFinding {
                        id: resp_id,
                        file,
                        line,
                        message,
                        severity,
                        rule_id,
                    } if resp_id == id => {
                        findings.push(GateFinding {
                            file,
                            line,
                            message,
                            severity: GateSeverity::from_str_lossy(&severity),
                            rule_id,
                        });
                    }
                    super::types::PluginResponse::GateDone {
                        id: resp_id,
                        passed,
                        summary,
                    } if resp_id == id => {
                        return Ok(GateOutcome {
                            passed,
                            gate_name: config.gate_name,
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
                "gate plugin '{}' exited before completing gate '{}'",
                self.instance.manifest().name,
                config.gate_name
            )))
        })
    }

    fn name(&self) -> &str {
        &self.instance.manifest().name
    }

    fn supported_gates(&self) -> &[String] {
        &self.supported_gates
    }
}

#[cfg(test)]
#[path = "gate_trait.tests.rs"]
mod tests;
