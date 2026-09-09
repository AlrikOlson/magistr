//! Hook dispatch — bridges lifecycle hooks to shell command execution.
//!
//! Provides a `ProcessHookDispatcher` that maps well-known magistr tool names
//! to shell commands and executes them. Used by both TUI and headless modes
//! to run lifecycle hooks during phase transitions.

use std::path::{Path, PathBuf};

use magistr_engine::{HookAction, HookDispatcher, HookError, HookOutput};

/// Maps magistr MCP tool names to shell commands and executes them.
/// Also directly executes `ShellCommand` hook actions.
///
/// For MCP tool hooks, knows about well-known tools (`magistr_format`,
/// `magistr_gate`) and delegates to the corresponding project commands.
/// Unknown MCP tools produce a warning rather than a hard failure.
///
/// For shell command hooks, runs the command directly with an optional timeout.
pub struct ProcessHookDispatcher {
    /// Project root directory (commands run here).
    project_dir: PathBuf,
    /// Format command (e.g., "cargo fmt").
    format_command: Option<String>,
    /// Quality gate commands.
    check_command: Option<String>,
    test_command: Option<String>,
    lint_command: Option<String>,
}

impl ProcessHookDispatcher {
    /// Create a dispatcher from project configuration.
    #[must_use]
    pub fn new(project_dir: &Path, config: &crate::types::ProjectConfig) -> Self {
        Self {
            project_dir: project_dir.to_owned(),
            format_command: Some(config.format.command.clone()),
            check_command: Some(config.quality_gate.check.clone()),
            test_command: Some(config.quality_gate.test.clone()),
            lint_command: Some(config.quality_gate.lint.clone()),
        }
    }

    /// Create a minimal dispatcher with no known commands (for testing).
    #[cfg(test)]
    pub fn empty(project_dir: &Path) -> Self {
        Self {
            project_dir: project_dir.to_owned(),
            format_command: None,
            check_command: None,
            test_command: None,
            lint_command: None,
        }
    }

    /// Resolve a tool name to a shell command, if known.
    fn resolve_command(&self, tool: &str) -> Option<&str> {
        // Normalize tool name: strip MCP prefixes like "mcp__magistr__"
        let bare = crate::tools::normalize::normalize_tool_name(tool);
        match bare.as_ref() {
            "magistr_format" => self.format_command.as_deref(),
            "magistr_gate" => self.check_command.as_deref(),
            "magistr_check" => self.check_command.as_deref(),
            "magistr_test" => self.test_command.as_deref(),
            "magistr_lint" => self.lint_command.as_deref(),
            _ => None,
        }
    }

    /// Execute a shell command in the project directory with an optional timeout.
    async fn run_command(
        &self,
        command: &str,
        timeout_secs: Option<u64>,
    ) -> Result<HookOutput, HookError> {
        let timeout = std::time::Duration::from_secs(timeout_secs.unwrap_or(30));

        let future = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(&self.project_dir)
            .output();

        let output = tokio::time::timeout(timeout, future)
            .await
            .map_err(|_| HookError {
                message: format!("Command `{command}` timed out after {}s", timeout.as_secs()),
            })?
            .map_err(|e| HookError {
                message: format!("Failed to spawn command: {e}"),
            })?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let message = if stdout.trim().is_empty() {
                None
            } else {
                Some(stdout.trim().to_string())
            };
            Ok(HookOutput { message })
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(HookError {
                message: format!(
                    "Command `{command}` failed (exit {}): {}",
                    output.status.code().unwrap_or(-1),
                    stderr.trim()
                ),
            })
        }
    }
}

impl HookDispatcher for ProcessHookDispatcher {
    async fn dispatch(&self, action: &HookAction) -> Result<HookOutput, HookError> {
        match action {
            HookAction::McpTool { tool, .. } => {
                if let Some(command) = self.resolve_command(tool) {
                    self.run_command(command, None).await
                } else {
                    // Unknown tool — succeed with a warning message rather than failing.
                    // The hook system is extensible; unknown tools are not an error.
                    Ok(HookOutput {
                        message: Some(format!(
                            "Hook tool '{tool}' has no local command mapping — skipped"
                        )),
                    })
                }
            }
            HookAction::ShellCommand {
                command,
                timeout_secs,
            } => self.run_command(command, *timeout_secs).await,
        }
    }
}

/// Execute lifecycle hooks for a given event in headless mode.
///
/// Loads hooks from project config, resolves those matching the event,
/// dispatches via `ProcessHookDispatcher`, and logs results to stderr.
pub async fn run_hooks_headless(
    project_dir: &Path,
    config: &crate::types::ProjectConfig,
    event: &magistr_engine::LifecycleEvent,
) {
    let resolved = magistr_engine::HookResolver::resolve(event, &config.lifecycle_hooks);
    if resolved.is_empty() {
        return;
    }

    let dispatcher = ProcessHookDispatcher::new(project_dir, config);
    let report = magistr_engine::HookExecutor::execute(&resolved, &dispatcher).await;

    for warning in report.warnings() {
        eprintln!("[hook] {warning}");
    }
    if !report.is_success() {
        eprintln!("[hook] lifecycle hooks for {event} failed");
    }
}

#[cfg(test)]
#[path = "hook_dispatch.tests.rs"]
mod tests;
