//! Workflow step plugin trait — contract for custom workflow step providers.
//!
//! Executable plugins of kind `WorkflowStep` implement this trait via the
//! JSON-line protocol. Workflow steps hook into the plan-execute-verify loop
//! at defined phases: security scans, code reviews, deploy hooks, notifications.

use std::future::Future;
use std::pin::Pin;
use std::time::{Duration, Instant};

use super::types::PluginError;

/// Phase in the plan-execute-verify loop where a workflow step can run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorkflowPhase {
    /// Before the research phase begins.
    PreResearch,
    /// After the research phase completes.
    PostResearch,
    /// Before the execute phase begins.
    PreExecute,
    /// After the execute phase completes.
    PostExecute,
    /// Before the verify phase begins.
    PreVerify,
    /// After the verify phase completes.
    PostVerify,
}

impl WorkflowPhase {
    /// Parse from a string (case-insensitive, underscores or hyphens).
    #[must_use]
    pub fn from_str_lossy(s: &str) -> Option<Self> {
        match s.to_lowercase().replace('-', "_").as_str() {
            "pre_research" => Some(Self::PreResearch),
            "post_research" => Some(Self::PostResearch),
            "pre_execute" => Some(Self::PreExecute),
            "post_execute" => Some(Self::PostExecute),
            "pre_verify" => Some(Self::PreVerify),
            "post_verify" => Some(Self::PostVerify),
            _ => None,
        }
    }

    /// Wire-format label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::PreResearch => "pre_research",
            Self::PostResearch => "post_research",
            Self::PreExecute => "pre_execute",
            Self::PostExecute => "post_execute",
            Self::PreVerify => "pre_verify",
            Self::PostVerify => "post_verify",
        }
    }
}

impl std::fmt::Display for WorkflowPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// Severity of a workflow step finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepSeverity {
    /// Blocks the step from passing.
    Error,
    /// Non-blocking but should be addressed.
    Warning,
    /// Informational only.
    Info,
}

impl StepSeverity {
    /// Parse from a string (case-insensitive).
    #[must_use]
    pub fn from_str_lossy(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "error" | "err" | "critical" | "blocker" => Self::Error,
            "warning" | "warn" => Self::Warning,
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

impl std::fmt::Display for StepSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// A single finding reported by a workflow step plugin.
#[derive(Debug, Clone, PartialEq)]
pub struct StepFinding {
    /// Human-readable finding message.
    pub message: String,
    /// Severity of the finding.
    pub severity: StepSeverity,
    /// File path where the finding was detected (if applicable).
    pub file: Option<String>,
    /// Line number in the file (if applicable).
    pub line: Option<u32>,
}

/// Configuration for running a workflow step.
#[derive(Debug, Clone)]
pub struct StepConfig {
    /// Step name to run (e.g., "security-scan", "code-review").
    pub step_name: String,
    /// Workflow phase.
    pub phase: WorkflowPhase,
    /// Project directory.
    pub project_dir: String,
    /// Optional step-specific configuration.
    pub config: Option<serde_json::Value>,
}

/// Outcome of running a workflow step.
#[derive(Debug, Clone, PartialEq)]
pub struct StepOutcome {
    /// Whether the step passed overall.
    pub passed: bool,
    /// Name of the step that was run.
    pub step_name: String,
    /// Phase the step ran in.
    pub phase: WorkflowPhase,
    /// Individual findings discovered during the step.
    pub findings: Vec<StepFinding>,
    /// Human-readable summary.
    pub summary: String,
    /// How long the step took to run.
    pub duration: Duration,
}

impl StepOutcome {
    /// Count findings by severity.
    #[must_use]
    pub fn count_by_severity(&self, severity: StepSeverity) -> usize {
        self.findings
            .iter()
            .filter(|f| f.severity == severity)
            .count()
    }

    /// Count error-severity findings.
    #[must_use]
    pub fn error_count(&self) -> usize {
        self.count_by_severity(StepSeverity::Error)
    }

    /// Count warning-severity findings.
    #[must_use]
    pub fn warning_count(&self) -> usize {
        self.count_by_severity(StepSeverity::Warning)
    }
}

/// Trait for workflow step plugins.
///
/// Workflow step plugins provide custom steps that hook into the
/// plan-execute-verify iteration loop: security scans, code reviews,
/// deployment hooks, notifications, etc.
///
/// Implementations communicate with plugin subprocesses via the JSON-line
/// protocol defined in [`super::types::PluginRequest`] and
/// [`super::types::PluginResponse`].
pub trait WorkflowStepPlugin: Send {
    /// Run the workflow step.
    ///
    /// Returns a [`StepOutcome`] with findings and a pass/fail verdict.
    fn run_step(
        &mut self,
        config: StepConfig,
    ) -> Pin<Box<dyn Future<Output = Result<StepOutcome, PluginError>> + Send + '_>>;

    /// Get the plugin name.
    fn name(&self) -> &str;

    /// Get the workflow phases this plugin supports.
    fn supported_phases(&self) -> &[WorkflowPhase];

    /// Get the step names this plugin can run.
    fn supported_steps(&self) -> &[String];
}

/// A workflow step plugin implementation backed by a [`PluginInstance`].
///
/// Translates trait method calls into JSON-line protocol requests and
/// parses the responses. Mirrors the [`super::gate_trait::SubprocessGate`]
/// pattern.
pub struct SubprocessWorkflowStep {
    /// The activated plugin instance.
    instance: super::lifecycle::PluginInstance,
    /// Phases reported by the plugin during init (from capabilities prefixed with `phase:`).
    supported_phases: Vec<WorkflowPhase>,
    /// Step names reported by the plugin during init (from capabilities prefixed with `step:`).
    supported_steps: Vec<String>,
}

impl SubprocessWorkflowStep {
    /// Create a new subprocess workflow step from an activated plugin instance.
    ///
    /// The instance must be in the `Active` state. Supported phases are
    /// extracted from capabilities prefixed with `phase:`, step names from `step:`.
    pub fn new(instance: super::lifecycle::PluginInstance) -> Result<Self, PluginError> {
        if instance.state() != super::lifecycle::PluginState::Active {
            return Err(PluginError::LifecycleViolation(format!(
                "workflow-step plugin '{}' must be active (current state: {})",
                instance.manifest().name,
                instance.state()
            )));
        }

        let supported_phases = instance
            .capabilities()
            .iter()
            .filter_map(|cap| {
                cap.strip_prefix("phase:")
                    .and_then(WorkflowPhase::from_str_lossy)
            })
            .collect();

        let supported_steps = instance
            .capabilities()
            .iter()
            .filter_map(|cap| cap.strip_prefix("step:").map(String::from))
            .collect();

        Ok(Self {
            instance,
            supported_phases,
            supported_steps,
        })
    }

    /// Get the underlying plugin instance.
    #[must_use]
    pub fn instance(&self) -> &super::lifecycle::PluginInstance {
        &self.instance
    }
}

impl WorkflowStepPlugin for SubprocessWorkflowStep {
    fn run_step(
        &mut self,
        config: StepConfig,
    ) -> Pin<Box<dyn Future<Output = Result<StepOutcome, PluginError>> + Send + '_>> {
        Box::pin(async move {
            let start = Instant::now();
            let id = self.instance.process_mut().next_id();
            let request = super::types::PluginRequest::WorkflowStepRun {
                id,
                step_name: config.step_name.clone(),
                phase: config.phase.label().to_string(),
                project_dir: config.project_dir,
                config: config.config,
            };

            self.instance.process_mut().send(&request)?;

            let mut findings = Vec::new();

            // Read streaming findings/progress until we get a StepDone or Error.
            while let Some(result) = self.instance.process_mut().recv().await {
                match result? {
                    super::types::PluginResponse::WorkflowStepFinding {
                        id: resp_id,
                        message,
                        severity,
                        file,
                        line,
                    } if resp_id == id => {
                        findings.push(StepFinding {
                            message,
                            severity: StepSeverity::from_str_lossy(&severity),
                            file,
                            line,
                        });
                    }
                    super::types::PluginResponse::WorkflowStepProgress { id: resp_id, .. }
                        if resp_id == id =>
                    {
                        // Progress updates are informational — continue waiting.
                    }
                    super::types::PluginResponse::WorkflowStepDone {
                        id: resp_id,
                        passed,
                        summary,
                    } if resp_id == id => {
                        return Ok(StepOutcome {
                            passed,
                            step_name: config.step_name,
                            phase: config.phase,
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
                    _ => continue,
                }
            }

            Err(PluginError::ProcessExited(format!(
                "workflow-step plugin '{}' exited before completing step '{}'",
                self.instance.manifest().name,
                config.step_name
            )))
        })
    }

    fn name(&self) -> &str {
        &self.instance.manifest().name
    }

    fn supported_phases(&self) -> &[WorkflowPhase] {
        &self.supported_phases
    }

    fn supported_steps(&self) -> &[String] {
        &self.supported_steps
    }
}

#[cfg(test)]
#[path = "workflow_step_trait.tests.rs"]
mod tests;
