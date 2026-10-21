//! Quality gate execution — async gate running and post-scaffold checks.
//!
//! Canonical type definitions (`Gate`, `GateResult`, `ValidationResult`) and
//! synchronous helpers (`load_project_config`, `gate_command`) live in
//! `magistr-core::validate`. This module re-exports them and provides async
//! execution functions that depend on the subprocess runtime.

use std::path::Path;
use std::time::Duration;

use crate::error::{MagistrError, Result};
use crate::types::QualityGate;
use crate::utils::subprocess::run_command_with_timeout;

#[path = "project_validation.rs"]
mod project_validation;
pub(crate) use project_validation::ProjectValidation;

// ── Re-exports from magistr-core ────────────────────────────────────────────

pub use magistr_core::validate::{
    Gate, GateResult, ValidationResult, gate_command, load_project_config,
};

// ── CLI extensions for core types ───────────────────────────────────────────

/// TUI-specific extensions for `GateResult`.
pub trait GateResultExt {
    /// Get the status symbol (`✓` for pass, `✗` for fail).
    fn status_symbol(&self) -> &'static str;
}

impl GateResultExt for GateResult {
    fn status_symbol(&self) -> &'static str {
        if self.passed { "\u{2713}" } else { "\u{2717}" }
    }
}

/// Maximum output size from gate commands (100 KB) to prevent overwhelming
/// LLM context with huge test/lint output.
const MAX_OUTPUT_BYTES: usize = 100_000;

/// Truncate command output to `max_bytes`, preserving head (80%) and tail (20%)
/// with a marker in between.
fn truncate_output(output: &str, max_bytes: usize) -> String {
    if output.len() <= max_bytes {
        return output.to_string();
    }
    let total = output.len();
    let head_budget = max_bytes * 4 / 5;
    let tail_budget = max_bytes - head_budget;

    let head_end = output
        .char_indices()
        .take_while(|(i, _)| *i <= head_budget)
        .last()
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);
    let tail_start = output
        .char_indices()
        .rev()
        .take_while(|(i, _)| total - *i <= tail_budget)
        .last()
        .map(|(i, _)| i)
        .unwrap_or(total);

    format!(
        "{}\n\n[... truncated, {} bytes total ...]\n\n{}",
        &output[..head_end],
        total,
        &output[tail_start..]
    )
}

/// Run a single quality gate command with timeout, graceful cleanup, and stdio isolation.
///
/// Delegates to [`crate::utils::subprocess::run_command_with_timeout`] which handles
/// process group management, SIGTERM→SIGKILL escalation, and orphan prevention.
pub async fn run_single_gate(gate: Gate, command: &str, dir: &Path) -> GateResult {
    if command.trim().is_empty() {
        return GateResult {
            gate,
            passed: false,
            duration: Duration::ZERO,
            output: "not configured; no check ran".into(),
            command: String::new(),
        };
    }

    let started = std::time::Instant::now();
    let policy = magistr_core::gates::execution_policy(dir, gate.label(), command, None);
    let (timeout, schedule) = match policy {
        Ok(policy) => policy,
        Err(reason) => return refused_gate(gate, command, reason, started.elapsed()),
    };
    let cancel = tokio_util::sync::CancellationToken::new();
    let _permit = match magistr_process::acquire_gate(
        dir,
        command,
        &schedule,
        Duration::from_secs(timeout),
        &cancel,
    )
    .await
    {
        Ok(permit) => permit,
        Err(reason) => {
            return refused_gate(
                gate,
                command,
                format!("Check did not start: {reason:?} while waiting for verification resources"),
                started.elapsed(),
            );
        }
    };
    let remaining = Duration::from_secs(timeout).saturating_sub(started.elapsed());
    let result =
        run_command_with_timeout(command, dir, remaining.as_secs_f64().ceil() as u64, None).await;

    let raw = if result.stderr.is_empty() {
        result.stdout
    } else if result.stdout.is_empty() {
        result.stderr
    } else {
        format!("{}\n{}", result.stdout, result.stderr)
    };

    GateResult {
        gate,
        passed: !result.timed_out && result.exit_code == 0,
        duration: started.elapsed(),
        output: truncate_output(&raw, MAX_OUTPUT_BYTES),
        command: command.to_string(),
    }
}

fn refused_gate(gate: Gate, command: &str, output: String, duration: Duration) -> GateResult {
    GateResult {
        gate,
        passed: false,
        duration,
        output,
        command: command.into(),
    }
}

/// Run quality gates under the shared resource limits.
pub async fn run_gates(dir: &Path) -> Result<ValidationResult> {
    let config = load_project_config(dir)?;
    run_gates_with_config(&config.quality_gate, dir).await
}

/// Quick post-scaffold validation: run the check command to verify the scaffold is correct.
///
/// This is called after `magistr init` to give immediate feedback on whether the
/// generated configuration matches the project. Only runs the `check` gate
/// (not tests or lint) to keep it fast.
///
/// Returns a tuple of (gate_name, command, passed, output).
pub async fn post_scaffold_check(qg: &QualityGate, dir: &Path) -> Vec<PostScaffoldResult> {
    let mut results = Vec::new();

    for gate in [Gate::Check] {
        let cmd = gate_command(qg, gate);
        if cmd.is_empty() {
            continue;
        }
        let gate_result = run_single_gate(gate, cmd, dir).await;
        results.push(PostScaffoldResult {
            gate,
            command: cmd.to_string(),
            passed: gate_result.passed,
            output: gate_result.output,
            duration: gate_result.duration,
        });
    }

    results
}

/// Result of a post-scaffold validation check.
#[derive(Debug, Clone)]
pub struct PostScaffoldResult {
    pub gate: Gate,
    pub command: String,
    pub passed: bool,
    pub output: String,
    pub duration: Duration,
}

impl PostScaffoldResult {
    pub fn status_icon(&self) -> String {
        if self.passed {
            crate::utils::format::icon_pass().to_string()
        } else {
            crate::utils::format::icon_fail().to_string()
        }
    }
}

/// Run all quality gates in parallel with an explicit config.
pub async fn run_gates_with_config(qg: &QualityGate, dir: &Path) -> Result<ValidationResult> {
    let mut specs = Gate::all()
        .into_iter()
        .map(|gate| magistr_core::gates::GateSpec {
            name: gate.label().into(),
            command: gate_command(qg, gate).into(),
            required: true,
        })
        .collect();
    magistr_core::gates::consolidate_gates(&mut specs);
    let mut handles = Vec::with_capacity(specs.len());

    for spec in specs {
        let gate = match spec.name.as_str() {
            "check" => Gate::Check,
            "lint" => Gate::Lint,
            _ => Gate::Test,
        };
        let cmd = spec.command;
        let dir = dir.to_path_buf();
        handles.push(tokio::spawn(async move {
            run_single_gate(gate, &cmd, &dir).await
        }));
    }

    let mut results = Vec::with_capacity(handles.len());
    for handle in handles {
        let result = handle
            .await
            .map_err(|e| MagistrError::TaskJoin(e.to_string()))?;
        results.push(result);
    }

    Ok(ValidationResult { results })
}

#[cfg(test)]
#[path = "validate.tests.rs"]
mod tests;
