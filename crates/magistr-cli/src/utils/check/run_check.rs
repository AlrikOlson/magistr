//! Run one check and keep everything it said.

use std::path::Path;

use tokio_util::sync::CancellationToken;

use super::check_run::CheckRun;
use super::check_verdict::CheckVerdict;
use super::truncate_output::{MAX_OUTPUT_BYTES, truncate_output};
use crate::utils::live_command::run_command_live;

/// Run `command` for the check `name` in `dir`, and answer with what it did.
///
/// The command runs on a pty ([`run_command_live`]): it sees a terminal,
/// and every task it writes — colours and cursor moves included — goes
/// to `sink` in order while it runs, which is how a caller streams the
/// check to the cockpit. What the run keeps is the escape-stripped
/// transcript, both streams as the one stream a tty is (a command that
/// writes diagnostics to stderr and progress to stdout is the normal
/// case, not the exception). Failure rows are parsed from the **full**
/// transcript before it is truncated, and a timeout, a cancel and a
/// non-zero exit are three distinguishable verdicts rather than one bool.
///
/// `cancel` is the caller's way out mid-check: when it fires, the command's
/// whole process group is ended the way a timeout ends it and the run comes
/// back [`CheckVerdict::Cancelled`]. A caller with nothing to cancel on
/// passes a fresh token, which never fires.
pub(crate) async fn run_check(
    name: &str,
    command: &str,
    required: bool,
    dir: &Path,
    timeout_secs: u64,
    cancel: &CancellationToken,
    sink: impl FnMut(&[u8]) + Send,
) -> CheckRun {
    if command.trim().is_empty() {
        let output = "not configured; no check ran".to_owned();
        let mut sink = sink;
        sink(output.as_bytes());
        return CheckRun {
            name: name.to_owned(),
            command: String::new(),
            required,
            verdict: CheckVerdict::Unconfigured,
            exit_code: -1,
            output,
            rows: Vec::new(),
            duration: std::time::Duration::ZERO,
            queue_duration: std::time::Duration::ZERO,
        };
    }
    let started = std::time::Instant::now();
    let policy = magistr_core::checks::execution_policy(
        dir,
        name,
        command,
        (timeout_secs != 0).then_some(timeout_secs),
    );
    let (timeout_secs, schedule) = match policy {
        Ok(policy) => policy,
        Err(reason) => {
            return refused(
                name,
                command,
                required,
                reason,
                CheckVerdict::LaunchFailed,
                started.elapsed(),
            );
        }
    };
    let _permit = match magistr_process::acquire_check(
        dir,
        command,
        &schedule,
        std::time::Duration::from_secs(timeout_secs),
        cancel,
    )
    .await
    {
        Ok(permit) => permit,
        Err(error) => {
            return refused(
                name,
                command,
                required,
                format!("Check did not start: {error:?} while waiting for verification resources"),
                if error == magistr_process::CheckWaitError::Cancelled {
                    CheckVerdict::Cancelled
                } else {
                    CheckVerdict::TimedOut
                },
                started.elapsed(),
            );
        }
    };
    let queued = started.elapsed();
    let remaining = std::time::Duration::from_secs(timeout_secs)
        .saturating_sub(queued)
        .as_secs_f64()
        .ceil() as u64;
    let result = run_command_live(command, dir, remaining.max(1), cancel, sink).await;

    let output = if result.stderr.is_empty() {
        result.stdout.clone()
    } else if result.stdout.is_empty() {
        result.stderr.clone()
    } else {
        format!("{}\n{}", result.stdout, result.stderr)
    };
    // Rows come from the full output: the diagnostic that matters can sit
    // past the truncation point.
    let rows = magistr_core::parse_check_rows(command, &output);

    let verdict = if result.cancelled {
        CheckVerdict::Cancelled
    } else if result.timed_out {
        CheckVerdict::TimedOut
    } else if result.exit_code == -1 {
        CheckVerdict::LaunchFailed
    } else if result.exit_code == 0 {
        CheckVerdict::Passed
    } else {
        CheckVerdict::Failed
    };

    CheckRun {
        name: name.to_owned(),
        command: command.to_owned(),
        required,
        verdict,
        exit_code: result.exit_code,
        output: truncate_output(&output, MAX_OUTPUT_BYTES),
        rows,
        duration: started.elapsed(),
        queue_duration: queued,
    }
}

fn refused(
    name: &str,
    command: &str,
    required: bool,
    output: String,
    verdict: CheckVerdict,
    duration: std::time::Duration,
) -> CheckRun {
    CheckRun {
        name: name.into(),
        command: command.into(),
        required,
        verdict,
        exit_code: -1,
        output,
        rows: vec![],
        duration,
        queue_duration: duration,
    }
}

#[cfg(test)]
#[path = "run_check.tests.rs"]
mod tests;
