//! What one check run actually did.

use std::time::Duration;

use magistr_core::CheckFailureRow;
use magistr_engine::native::CheckExecution;

use super::check_verdict::CheckVerdict;

/// How much of a passing check's output the trace keeps.
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

/// One check's run, whole.
///
/// This is the evidence both callers keep: the MCP path renders it as JSON
/// and forwards it to `check_step`, and the loop driver carries it as its
/// `CheckOutcome`. It replaces the driver's old three-field record, whose
/// `passed: bool` was the whole account of a 153-second `cargo nextest`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CheckRun {
    /// The check's name (`check`, `test`, `lint`, a custom check).
    pub(crate) name: String,
    /// The command that was run, verbatim — a record of the result is a
    /// record of what produced it.
    pub(crate) command: String,
    /// Whether a red here is a real failure. A check registered
    /// not-required is expected to fail and is neither retried nor acted on.
    pub(crate) required: bool,
    /// What the run decided.
    pub(crate) verdict: CheckVerdict,
    /// The code the process exited with (-1 when it was killed).
    pub(crate) exit_code: i32,
    /// The escape-stripped transcript — both streams, as the one stream
    /// the check's pty is — truncated at [`MAX_OUTPUT_BYTES`].
    ///
    /// [`MAX_OUTPUT_BYTES`]: super::MAX_OUTPUT_BYTES
    pub(crate) output: String,
    /// The failures parsed out of the full output, before truncation.
    pub(crate) rows: Vec<CheckFailureRow>,
    /// Wall-clock time including the resource queue.
    pub(crate) duration: Duration,
    /// Time waiting for verification resources before the child started.
    pub(crate) queue_duration: Duration,
}

impl CheckRun {
    /// How the command ended, for the check's receipt: only an exit is a
    /// statement about the code, and
    /// a timeout or a cancel is recorded as exactly that.
    pub(crate) fn execution(&self) -> CheckExecution {
        match self.verdict {
            CheckVerdict::Passed | CheckVerdict::Failed => CheckExecution::Exited {
                code: self.exit_code,
            },
            CheckVerdict::Unconfigured => CheckExecution::LaunchFailed {
                reason: "no command configured".into(),
            },
            CheckVerdict::LaunchFailed => CheckExecution::LaunchFailed {
                reason: self.output.clone(),
            },
            CheckVerdict::TimedOut => CheckExecution::TimedOut,
            CheckVerdict::Cancelled => CheckExecution::Cancelled,
        }
    }

    /// Whether this run lets the loop go on.
    pub(crate) const fn passed(&self) -> bool {
        self.verdict.passed()
    }

    /// Retain bounded passing output and the full captured failure diagnostics.
    pub(crate) fn details(&self) -> String {
        if self.verdict == CheckVerdict::Passed {
            tail(&self.output, GREEN_DETAIL_BYTES)
        } else {
            self.output.clone()
        }
    }

    /// The one line the cockpit's job result shows for this run.
    pub(crate) fn line(&self) -> String {
        let secs = self.duration.as_secs_f64();
        match self.verdict {
            CheckVerdict::Passed => format!("{} passed in {secs:.1}s", self.name),
            CheckVerdict::Unconfigured => format!("{} is not configured; no check ran", self.name),
            CheckVerdict::LaunchFailed => format!("{} could not start: {}", self.name, self.output),
            CheckVerdict::Failed => {
                format!(
                    "{} failed (exit {}) in {secs:.1}s",
                    self.name, self.exit_code
                )
            }
            CheckVerdict::TimedOut => format!("{} timed out after {secs:.0}s", self.name),
            CheckVerdict::Cancelled => format!("{} cancelled after {secs:.1}s", self.name),
        }
    }
}

#[cfg(test)]
#[path = "check_run.tests.rs"]
mod tests;
