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

// ── TUI extensions for core types ───────────────────────────────────────────

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

/// Default timeout for gate commands (5 minutes).
const DEFAULT_GATE_TIMEOUT_SECS: u64 = 300;

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
    if command.is_empty() {
        return GateResult {
            gate,
            passed: true,
            duration: Duration::ZERO,
            output: "skipped (no command configured)".into(),
            command: String::new(),
        };
    }

    let result = run_command_with_timeout(command, dir, DEFAULT_GATE_TIMEOUT_SECS, None).await;

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
        duration: result.duration,
        output: truncate_output(&raw, MAX_OUTPUT_BYTES),
        command: command.to_string(),
    }
}

/// Run all quality gates in parallel.
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

    for gate in Gate::all() {
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
    let gates = Gate::all();
    let mut handles = Vec::with_capacity(gates.len());

    for gate in gates {
        let cmd = gate_command(qg, gate).to_string();
        let dir = dir.to_path_buf();
        handles.push(tokio::spawn(async move {
            run_single_gate(gate, &cmd, &dir).await
        }));
    }

    let mut results = Vec::with_capacity(gates.len());
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
