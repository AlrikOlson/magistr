//! Quality gate result types.
//!
//! Frontend-agnostic representations of quality gate outcomes (check, test,
//! lint). Used by both desktop and command-line consumers to display gate status, history sparklines,
//! and transition summaries.

use super::gate_rows::GateFailureRow;

/// Result of a single quality gate for display.
#[derive(Debug, Clone)]
pub struct GateResult {
    /// Gate name (e.g., "check", "test", "lint").
    pub name: String,
    /// Whether the gate passed.
    pub passed: bool,
    /// Duration in seconds.
    pub duration_secs: f64,
    /// Truncated output (first few lines of stdout/stderr) for expandable details.
    pub output: Option<String>,
    /// Number of tests/checks in this gate (for sparkline height).
    /// `None` means unknown (sparkline uses duration as fallback).
    pub test_count: Option<u32>,
    /// Structured failures parsed from the output (`gate_rows`): what the
    /// panel lists and what a click opens. Empty when nothing parsed.
    pub rows: Vec<GateFailureRow>,
}

/// Compact gate result for transition cards (avoids cloning full output).
#[derive(Debug, Clone)]
pub struct GateSummary {
    /// Gate name (e.g., "check", "test", "lint").
    pub name: String,
    /// Whether the gate passed.
    pub passed: bool,
    /// Duration in seconds.
    pub duration_secs: f64,
}

/// Outcome of a completed loop iteration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IterationOutcome {
    /// All quality gates passed.
    Passed,
    /// One or more quality gates failed.
    Failed,
}

/// Phase color for a completed iteration segment.
///
/// Each completed iteration ended in a particular phase, which determines
/// its segment color in the progress bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IterationPhase {
    /// Iteration completed during/after research.
    Research,
    /// Iteration completed during/after planning.
    Plan,
    /// Iteration completed during/after execution.
    Execute,
    /// Iteration completed during/after verification.
    Verify,
    /// Iteration completed after quality gates passed.
    Gate,
}

impl IterationPhase {
    /// Human-readable label for this phase.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Research => "research",
            Self::Plan => "plan",
            Self::Execute => "execute",
            Self::Verify => "verify",
            Self::Gate => "gate",
        }
    }
}

/// Maximum number of gate result snapshots to keep for sparkline history.
pub const GATE_HISTORY_MAX: usize = 10;

#[cfg(test)]
#[path = "gate.tests.rs"]
mod tests;
