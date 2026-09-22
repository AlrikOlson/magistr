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
    /// On a [`GateVerdict::Flaky`] run, the code the *failing* attempt
    /// exited with. `None` on every other verdict.
    pub(crate) first_exit_code: Option<i32>,
    /// The escape-stripped transcript — both streams, as the one stream
    /// the gate's pty is — truncated at [`MAX_OUTPUT_BYTES`].
    ///
    /// [`MAX_OUTPUT_BYTES`]: super::MAX_OUTPUT_BYTES
    pub(crate) output: String,
    /// The failures parsed out of the full output, before truncation.
    pub(crate) rows: Vec<GateFailureRow>,
    /// Wall-clock time, both attempts summed on a flake.
    pub(crate) duration: Duration,
}

impl GateRun {
    /// How the command ended, for the check's receipt: only an exit is a
    /// statement about the code (a flake is its passing retry's exit), and
    /// a timeout or a cancel is recorded as exactly that.
    pub(crate) fn execution(&self) -> CheckExecution {
        match self.verdict {
            GateVerdict::Passed | GateVerdict::Failed | GateVerdict::Flaky => {
                CheckExecution::Exited {
                    code: self.exit_code,
                }
            }
            GateVerdict::Unconfigured => CheckExecution::LaunchFailed {
                reason: "no command configured".into(),
            },
            GateVerdict::TimedOut => CheckExecution::TimedOut,
            GateVerdict::Cancelled => CheckExecution::Cancelled,
        }
    }

    /// Whether this run lets the loop go on.
    pub(crate) const fn passed(&self) -> bool {
        self.verdict.passed()
    }

    /// Whether a red here is worth spending a second run on: a required
    /// gate that exited non-zero, and nothing else.
    ///
    /// A timeout would cost another whole timeout for the same answer; a
    /// cancel means the cockpit is closing and there is nothing to retry
    /// into; a not-required gate is one the project expects to fail, so
    /// paying for it twice buys nothing.
    pub(crate) const fn is_retryable(&self) -> bool {
        self.required && matches!(self.verdict, GateVerdict::Failed)
    }

    /// This (passing) run recast as the successful half of a flake, with
    /// `failed`'s exit code, output and rows kept — the green attempt has
    /// nothing to say, and the red one is the whole evidence of the flake.
    pub(crate) fn as_flake_of(&self, failed: &Self) -> Self {
        Self {
            verdict: GateVerdict::Flaky,
            first_exit_code: Some(failed.exit_code),
            output: failed.output.clone(),
            rows: failed.rows.clone(),
            duration: self.duration.saturating_add(failed.duration),
            ..self.clone()
        }
    }

    /// What the execution trace keeps for this run.
    ///
    /// Asymmetric on purpose. A red or flaky run persists its whole
    /// captured output — that is the evidence the record exists for. A
    /// green one persists only its tail: a passing `cargo nextest` on a
    /// large workspace emits hundreds of KB of PASS lines nobody will
    /// read, the store is re-read and merge-written whole on every
    /// mutation, and a green run's entire evidentiary content is its
    /// summary line. A flake leads with both exit codes, because it is
    /// recorded as passed with the retry's zero — without that line the
    /// flake would be invisible in the record.
    pub(crate) fn details(&self) -> String {
        let body = if self.verdict == GateVerdict::Passed {
            tail(&self.output, GREEN_DETAIL_BYTES)
        } else {
            self.output.clone()
        };
        match self.first_exit_code {
            Some(first) => format!(
                "flaky: exit {first}, then exit {} on the retry\n{body}",
                self.exit_code
            ),
            None => body,
        }
    }

    /// The one line the cockpit's job result shows for this run.
    pub(crate) fn line(&self) -> String {
        let secs = self.duration.as_secs_f64();
        match self.verdict {
            GateVerdict::Passed => format!("{} passed in {secs:.1}s", self.name),
            GateVerdict::Unconfigured => format!("{} is not configured; no check ran", self.name),
            GateVerdict::Failed => {
                format!(
                    "{} failed (exit {}) in {secs:.1}s",
                    self.name, self.exit_code
                )
            }
            GateVerdict::TimedOut => format!("{} timed out after {secs:.0}s", self.name),
            GateVerdict::Cancelled => format!("{} cancelled after {secs:.1}s", self.name),
            GateVerdict::Flaky => format!(
                "{} flaky: failed (exit {}) then passed on the retry, {secs:.1}s",
                self.name,
                self.first_exit_code.unwrap_or(-1)
            ),
        }
    }
}

#[cfg(test)]
#[path = "gate_run.tests.rs"]
mod tests;
