//! What one gate run actually did.

use std::time::Duration;

use magistr_core::GateFailureRow;
use magistr_engine::native::CheckExecution;

use super::gate_verdict::GateVerdict;

/// How much of a passing gate's output the trace keeps.
const GREEN_DETAIL_BYTES: usize = 2_000;

/// The last `max_bytes` of `text`, cut on a char boundary and marked.
fn tail(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_owned();
    }
    let start = text
        .char_indices()
        .rev()
        .take_while(|(i, _)| text.len() - *i <= max_bytes)
        .last()
        .map_or(text.len(), |(i, _)| i);
    format!("[… {} bytes, tail only …]\n{}", text.len(), &text[start..])
}

/// One gate's run, whole.
///
/// This is the evidence both callers keep: the MCP path renders it as JSON
/// and forwards it to `ship_check`, and the loop driver carries it as its
/// `GateOutcome`. It replaces the driver's old three-field record, whose
/// `passed: bool` was the whole account of a 153-second `cargo nextest`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GateRun {
    /// The gate's name (`check`, `test`, `lint`, a custom gate).
    pub(crate) name: String,
    /// The command that was run, verbatim — a record of the result is a
    /// record of what produced it.
    pub(crate) command: String,
    /// Whether a red here is a real failure. A gate registered
    /// not-required is expected to fail and is neither retried nor acted on.
    pub(crate) required: bool,
    /// What the run decided.
    pub(crate) verdict: GateVerdict,
    /// The code the process exited with (-1 when it was killed).
    pub(crate) exit_code: i32,
    /// The escape-stripped transcript — both streams, as the one stream
    /// the gate's pty is — truncated at [`MAX_OUTPUT_BYTES`].
    ///
    /// [`MAX_OUTPUT_BYTES`]: super::MAX_OUTPUT_BYTES
    pub(crate) output: String,
    /// The failures parsed out of the full output, before truncation.
    pub(crate) rows: Vec<GateFailureRow>,
    /// Wall-clock time including the resource queue.
    pub(crate) duration: Duration,
    /// Time waiting for verification resources before the child started.
    pub(crate) queue_duration: Duration,
}

impl GateRun {
    /// How the command ended, for the check's receipt: only an exit is a
    /// statement about the code, and
    /// a timeout or a cancel is recorded as exactly that.
    pub(crate) fn execution(&self) -> CheckExecution {
        match self.verdict {
            GateVerdict::Passed | GateVerdict::Failed => CheckExecution::Exited {
                code: self.exit_code,
            },
            GateVerdict::Unconfigured => CheckExecution::LaunchFailed {
                reason: "no command configured".into(),
            },
            GateVerdict::LaunchFailed => CheckExecution::LaunchFailed {
                reason: self.output.clone(),
            },
            GateVerdict::TimedOut => CheckExecution::TimedOut,
            GateVerdict::Cancelled => CheckExecution::Cancelled,
        }
    }

    /// Whether this run lets the loop go on.
    pub(crate) const fn passed(&self) -> bool {
        self.verdict.passed()
    }

    /// Retain bounded passing output and the full captured failure diagnostics.
    pub(crate) fn details(&self) -> String {
        if self.verdict == GateVerdict::Passed {
            tail(&self.output, GREEN_DETAIL_BYTES)
        } else {
            self.output.clone()
        }
    }

    /// The one line the cockpit's job result shows for this run.
    pub(crate) fn line(&self) -> String {
        let secs = self.duration.as_secs_f64();
        match self.verdict {
            GateVerdict::Passed => format!("{} passed in {secs:.1}s", self.name),
            GateVerdict::Unconfigured => format!("{} is not configured; no check ran", self.name),
            GateVerdict::LaunchFailed => format!("{} could not start: {}", self.name, self.output),
            GateVerdict::Failed => {
                format!(
                    "{} failed (exit {}) in {secs:.1}s",
                    self.name, self.exit_code
                )
            }
            GateVerdict::TimedOut => format!("{} timed out after {secs:.0}s", self.name),
            GateVerdict::Cancelled => format!("{} cancelled after {secs:.1}s", self.name),
        }
    }
}

#[cfg(test)]
#[path = "gate_run.tests.rs"]
mod tests;
