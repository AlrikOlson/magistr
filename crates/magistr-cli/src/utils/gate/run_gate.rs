//! Run one gate and keep everything it said.

use std::path::Path;

use tokio_util::sync::CancellationToken;

use super::gate_run::GateRun;
use super::gate_verdict::GateVerdict;
use super::truncate_output::{MAX_OUTPUT_BYTES, truncate_output};
use crate::utils::live_command::run_command_live;

/// Run `command` for the gate `name` in `dir`, and answer with what it did.
///
/// The command runs on a pty ([`run_command_live`]): it sees a terminal,
/// and every chunk it writes — colours and cursor moves included — goes
/// to `sink` in order while it runs, which is how a caller streams the
/// gate to the cockpit. What the run keeps is the escape-stripped
/// transcript, both streams as the one stream a tty is (a command that
/// writes diagnostics to stderr and progress to stdout is the normal
/// case, not the exception). Failure rows are parsed from the **full**
/// transcript before it is truncated, and a timeout, a cancel and a
/// non-zero exit are three distinguishable verdicts rather than one bool.
///
/// `cancel` is the caller's way out mid-gate: when it fires, the command's
/// whole process group is ended the way a timeout ends it and the run comes
/// back [`GateVerdict::Cancelled`]. A caller with nothing to cancel on
/// passes a fresh token, which never fires.
pub(crate) async fn run_gate(
    name: &str,
    command: &str,
    required: bool,
    dir: &Path,
    timeout_secs: u64,
    cancel: &CancellationToken,
    sink: impl FnMut(&[u8]) + Send,
) -> GateRun {
    if command.trim().is_empty() {
        let output = "not configured; no check ran".to_owned();
        let mut sink = sink;
        sink(output.as_bytes());
        return GateRun {
            name: name.to_owned(),
            command: String::new(),
            required,
            verdict: GateVerdict::Unconfigured,
            exit_code: -1,
            first_exit_code: None,
            output,
            rows: Vec::new(),
            duration: std::time::Duration::ZERO,
        };
    }
    let result = run_command_live(command, dir, timeout_secs, cancel, sink).await;

    let output = if result.stderr.is_empty() {
        result.stdout.clone()
    } else if result.stdout.is_empty() {
        result.stderr.clone()
    } else {
        format!("{}\n{}", result.stdout, result.stderr)
    };
    // Rows come from the full output: the diagnostic that matters can sit
    // past the truncation point.
    let rows = magistr_core::parse_gate_rows(command, &output);

    let verdict = if result.cancelled {
        GateVerdict::Cancelled
    } else if result.timed_out {
        GateVerdict::TimedOut
    } else if result.exit_code == 0 {
        GateVerdict::Passed
    } else {
        GateVerdict::Failed
    };

    GateRun {
        name: name.to_owned(),
        command: command.to_owned(),
        required,
        verdict,
        exit_code: result.exit_code,
        first_exit_code: None,
        output: truncate_output(&output, MAX_OUTPUT_BYTES),
        rows,
        duration: result.duration,
    }
}

#[cfg(test)]
#[path = "run_gate.tests.rs"]
mod tests;
