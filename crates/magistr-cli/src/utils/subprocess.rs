//! Shell lines run on a caller's behalf, over Magistr's owned-process
//! mechanism ([`magistr_process`]).
//!
//! `run_command_with_timeout()` runs `sh -c <command>` in its own process
//! group under a wall-clock cap; `run_command_cancellable()` additionally
//! races the child against a [`CancellationToken`]. Either way the group
//! is ended gracefully — `SIGTERM`, [`SIGTERM_GRACE`], `SIGKILL`, reap — and
//! the group is registered with [`super::child_guard`] for the life of the
//! run so a hard exit still cleans it up. What is decided here is the
//! *admission*: the line is the configured gate command a caller resolved,
//! and it is always a shell line, never an argv.

use std::path::Path;
use std::time::{Duration, Instant};

use magistr_process::{Launch, OwnedProcess, ProcessOutcome, ProcessSpec, Termination};
use tokio_util::sync::CancellationToken;

/// Result of running a shell command.
#[derive(Debug, Clone)]
pub struct CommandResult {
    /// Process exit code (-1 if unavailable, e.g. killed by signal).
    pub exit_code: i32,
    /// Captured stdout.
    pub stdout: String,
    /// Captured stderr.
    pub stderr: String,
    /// Wall-clock duration.
    pub duration: Duration,
    /// Whether the command was killed due to timeout.
    pub timed_out: bool,
    /// Whether the command was killed because the run was cancelled.
    pub cancelled: bool,
}

/// Callback invoked periodically during long-running commands.
///
/// Receives the elapsed duration since the command started.
pub type ProgressCallback = Box<dyn Fn(Duration) + Send + 'static>;

/// Default heartbeat interval for progress callbacks.
const DEFAULT_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(10);

/// Grace period between SIGTERM and SIGKILL on a timeout or a cancel.
pub(crate) const SIGTERM_GRACE: Duration = Duration::from_secs(5);

/// Run a shell command with timeout, graceful process cleanup, and optional progress heartbeat.
///
/// - Spawns `sh -c <command>` in its own process group
/// - Registers the group with the global child guard for cleanup on hard exit
/// - Captures stdout/stderr under the mechanism's ceiling
/// - On timeout: sends SIGTERM to the process group, waits [`SIGTERM_GRACE`],
///   then SIGKILL if still alive, then reaps
/// - If `progress` is provided, calls it every 10s with elapsed duration
pub async fn run_command_with_timeout(
    command: &str,
    dir: &Path,
    timeout_secs: u64,
    progress: Option<ProgressCallback>,
) -> CommandResult {
    run_command_with_timeout_interval(
        command,
        dir,
        timeout_secs,
        progress,
        DEFAULT_HEARTBEAT_INTERVAL,
    )
    .await
}

/// Like [`run_command_with_timeout`] but with a configurable heartbeat interval.
///
/// Exposed for testing with shorter intervals.
pub async fn run_command_with_timeout_interval(
    command: &str,
    dir: &Path,
    timeout_secs: u64,
    progress: Option<ProgressCallback>,
    heartbeat_interval: Duration,
) -> CommandResult {
    run_core(
        command,
        dir,
        timeout_secs,
        progress,
        heartbeat_interval,
        &CancellationToken::new(),
    )
    .await
}

/// Like [`run_command_with_timeout`], additionally racing the child
/// against `cancel`: when the token fires first, the whole process group
/// is ended the same way a timeout ends it (SIGTERM, [`SIGTERM_GRACE`],
/// SIGKILL, reap) and the result comes back with `cancelled` set.
pub async fn run_command_cancellable(
    command: &str,
    dir: &Path,
    timeout_secs: u64,
    progress: Option<ProgressCallback>,
    cancel: &CancellationToken,
) -> CommandResult {
    run_core(
        command,
        dir,
        timeout_secs,
        progress,
        DEFAULT_HEARTBEAT_INTERVAL,
        cancel,
    )
    .await
}

/// The spec every shell line Magistr runs for a caller gets: the line as
/// written, `dir`, the cap, pipes, and the graceful ending.
pub(crate) fn shell_spec(command: &str, dir: &Path, timeout_secs: u64) -> ProcessSpec {
    ProcessSpec::new(
        Launch::Shell(command.to_owned()),
        Duration::from_secs(timeout_secs),
    )
    .in_dir(dir)
    .with_termination(Termination::Graceful {
        grace: SIGTERM_GRACE,
    })
}

/// Run a spawned process with its group registered in the child guard for
/// the duration, so a hard exit of this process ends it too — and under
/// the process-wide [`Shutdown`](super::shutdown::Shutdown) request, so an
/// interrupt ends the group the way the caller's own cancel would
/// (`SIGTERM`, the grace, `SIGKILL`, reap) and the run answers `cancelled`.
pub(crate) async fn run_guarded(
    process: OwnedProcess,
    cancel: &CancellationToken,
    sink: impl FnMut(&[u8]) + Send,
) -> ProcessOutcome {
    let (cancel, _bound) = super::shutdown::Shutdown::bound(cancel);
    process.run(&cancel, sink).await
}

/// The answer for a gate with no command configured: nothing ran.
pub(crate) fn skipped() -> CommandResult {
    CommandResult {
        exit_code: 0,
        stdout: "skipped (no command configured)".into(),
        stderr: String::new(),
        duration: Duration::ZERO,
        timed_out: false,
        cancelled: false,
    }
}

/// The answer for a launch that never started.
pub(crate) fn spawn_failed(error: &std::io::Error) -> CommandResult {
    CommandResult {
        exit_code: -1,
        stdout: String::new(),
        stderr: format!("failed to spawn: {error}"),
        duration: Duration::ZERO,
        timed_out: false,
        cancelled: false,
    }
}

/// The gate's account of an outcome: partial output kept, and how the
/// run ended named on stderr when it was not the child's own exit.
pub(crate) fn command_result(outcome: ProcessOutcome, timeout_secs: u64) -> CommandResult {
    let exit_code = if outcome.fault.is_some() {
        -1
    } else {
        outcome.exit_code.unwrap_or(-1)
    };
    let mut stderr = outcome.stderr;
    let ending = if outcome.cancelled {
        Some("cancelled".to_owned())
    } else if outcome.timed_out {
        Some(format!("timed out after {timeout_secs}s"))
    } else {
        None
    };
    if let Some(ending) = ending {
        if !stderr.is_empty() && !stderr.ends_with('\n') {
            stderr.push('\n');
        }
        stderr.push_str(&ending);
    }
    if let Some(fault) = outcome.fault {
        if !stderr.is_empty() && !stderr.ends_with('\n') {
            stderr.push('\n');
        }
        stderr.push_str(&format!("failed to execute: {fault}"));
    }
    CommandResult {
        exit_code,
        stdout: outcome.stdout,
        stderr,
        duration: outcome.duration,
        timed_out: outcome.timed_out,
        cancelled: outcome.cancelled,
    }
}

async fn run_core(
    command: &str,
    dir: &Path,
    timeout_secs: u64,
    progress: Option<ProgressCallback>,
    heartbeat_interval: Duration,
    cancel: &CancellationToken,
) -> CommandResult {
    if command.is_empty() {
        return skipped();
    }
    let process = match OwnedProcess::spawn(shell_spec(command, dir, timeout_secs)) {
        Ok(process) => process,
        Err(error) => return spawn_failed(&error),
    };
    let heartbeat = progress.map(|callback| {
        let start = Instant::now();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(heartbeat_interval);
            interval.tick().await; // first tick is immediate — skip it
            loop {
                interval.tick().await;
                callback(start.elapsed());
            }
        })
    });
    let outcome = run_guarded(process, cancel, |_| {}).await;
    if let Some(heartbeat) = heartbeat {
        heartbeat.abort();
    }
    command_result(outcome, timeout_secs)
}

#[cfg(test)]
#[path = "subprocess.tests.rs"]
mod tests;
