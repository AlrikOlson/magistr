//! Reusable subprocess runner with graceful cleanup.
//!
//! `run_command_with_timeout()` encapsulates the full lifecycle:
//! spawn → read stdout/stderr → timeout → SIGTERM → wait → SIGKILL → reap.
//! `run_command_cancellable()` additionally races the child against a
//! [`CancellationToken`]; cancellation ends the process group the same
//! way a timeout does.
//!
//! All child processes are spawned in their own process group (`setsid`) and
//! registered with [`super::child_guard`] so the signal handler can clean up
//! on hard exit.

use std::path::Path;
use std::time::{Duration, Instant};

use tokio::process::Command;
use tokio_util::sync::CancellationToken;

use std::process::Stdio;

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

/// Grace period between SIGTERM and SIGKILL on timeout.
#[cfg(unix)]
const SIGTERM_GRACE_SECS: u64 = 5;

/// Run a shell command with timeout, graceful process cleanup, and optional progress heartbeat.
///
/// - Spawns `sh -c <command>` in its own process group (via `setsid`)
/// - Registers the PGID with the global child guard for cleanup on hard exit
/// - Pipes stdout/stderr (reads concurrently to avoid pipe buffer deadlock)
/// - On timeout: sends SIGTERM to process group, waits [`SIGTERM_GRACE_SECS`],
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
/// is ended the same way a timeout ends it (SIGTERM, [`SIGTERM_GRACE_SECS`],
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

/// What the race between the child, the clock and the caller decided.
enum Waited {
    /// The child ended on its own: its status and captured output.
    Finished(std::io::Result<std::process::ExitStatus>, Vec<u8>, Vec<u8>),
    TimedOut,
    Cancelled,
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
        return CommandResult {
            exit_code: 0,
            stdout: "skipped (no command configured)".into(),
            stderr: String::new(),
            duration: Duration::ZERO,
            timed_out: false,
            cancelled: false,
        };
    }

    let start = Instant::now();
    let timeout_dur = Duration::from_secs(timeout_secs);

    // Spawn in its own process group so we can kill the entire tree.
    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg(command)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // On Unix: use setsid to create a new process group.
    #[cfg(unix)]
    unsafe {
        cmd.pre_exec(|| {
            // Create new session (and process group). The child becomes the
            // process group leader, so killpg() reaches all descendants.
            libc::setsid();
            Ok(())
        });
    }

    let spawn_result = cmd.spawn();

    let mut child = match spawn_result {
        Ok(child) => child,
        Err(e) => {
            return CommandResult {
                exit_code: -1,
                stdout: String::new(),
                stderr: format!("failed to spawn: {e}"),
                duration: start.elapsed(),
                timed_out: false,
                cancelled: false,
            };
        }
    };

    // Register PGID for cleanup on hard exit (Ctrl+C).
    #[cfg(unix)]
    let pgid = child.id().map(|pid| pid as i32).unwrap_or(0);
    #[cfg(unix)]
    super::child_guard::register_child_pgid(pgid);

    // Take pipes before borrowing child into the raced future.
    let child_stdout = child.stdout.take();
    let child_stderr = child.stderr.take();

    // Spawn optional heartbeat task.
    let heartbeat_handle = progress.map(|cb| {
        let start_time = start;
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(heartbeat_interval);
            interval.tick().await; // first tick is immediate — skip it
            loop {
                interval.tick().await;
                cb(start_time.elapsed());
            }
        })
    });

    // Wait for completion, timeout or cancellation, reading output
    // concurrently. The block scopes the child's borrow so the kill
    // paths below can take it again.
    let waited = {
        let work = async {
            tokio::join!(
                child.wait(),
                read_pipe_stdout(child_stdout),
                read_pipe_stderr(child_stderr),
            )
        };
        tokio::pin!(work);
        tokio::select! {
            result = tokio::time::timeout(timeout_dur, &mut work) => match result {
                Ok((status, stdout, stderr)) => Waited::Finished(status, stdout, stderr),
                Err(_) => Waited::TimedOut,
            },
            () = cancel.cancelled() => Waited::Cancelled,
        }
    };

    // Cancel heartbeat.
    if let Some(handle) = heartbeat_handle {
        handle.abort();
    }

    let duration = start.elapsed();

    // Unregister PGID.
    #[cfg(unix)]
    super::child_guard::unregister_child_pgid(pgid);

    match waited {
        Waited::Finished(Ok(status), stdout_bytes, stderr_bytes) => {
            let stdout = String::from_utf8_lossy(&stdout_bytes).into_owned();
            let stderr = String::from_utf8_lossy(&stderr_bytes).into_owned();
            CommandResult {
                exit_code: status.code().unwrap_or(-1),
                stdout,
                stderr,
                duration,
                timed_out: false,
                cancelled: false,
            }
        }
        Waited::Finished(Err(e), _, _) => CommandResult {
            exit_code: -1,
            stdout: String::new(),
            stderr: format!("failed to execute: {e}"),
            duration,
            timed_out: false,
            cancelled: false,
        },
        Waited::TimedOut => {
            // Timeout: graceful SIGTERM → wait → SIGKILL
            graceful_kill(
                &mut child,
                #[cfg(unix)]
                pgid,
            )
            .await;
            CommandResult {
                exit_code: -1,
                stdout: String::new(),
                stderr: format!("timed out after {timeout_secs}s"),
                duration,
                timed_out: true,
                cancelled: false,
            }
        }
        Waited::Cancelled => {
            // Cancelled: the same graceful end, reported as such.
            graceful_kill(
                &mut child,
                #[cfg(unix)]
                pgid,
            )
            .await;
            CommandResult {
                exit_code: -1,
                stdout: String::new(),
                stderr: "cancelled".into(),
                duration,
                timed_out: false,
                cancelled: true,
            }
        }
    }
}

/// Graceful kill: SIGTERM the process group, wait, then SIGKILL if needed.
/// Shared with the pty runner (`live_command`), so a gate ends the same
/// way whichever way it was captured.
pub(crate) async fn graceful_kill(child: &mut tokio::process::Child, #[cfg(unix)] pgid: i32) {
    #[cfg(unix)]
    {
        // Send SIGTERM to the entire process group.
        if pgid > 0 {
            // SAFETY: killpg is safe to call with a valid PGID.
            unsafe {
                libc::killpg(pgid, libc::SIGTERM);
            }
        }

        // Wait up to SIGTERM_GRACE_SECS for graceful exit.
        let grace = Duration::from_secs(SIGTERM_GRACE_SECS);
        match tokio::time::timeout(grace, child.wait()).await {
            Ok(_) => (), // Exited gracefully after SIGTERM
            Err(_) => {
                // Still alive — escalate to SIGKILL on the process group.
                if pgid > 0 {
                    unsafe {
                        libc::killpg(pgid, libc::SIGKILL);
                    }
                }
                // Reap the zombie.
                let _ = child.wait().await;
            }
        }
    }

    #[cfg(not(unix))]
    {
        // On non-Unix, just kill directly (no process groups).
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
}

/// Read all bytes from an optional child stdout pipe.
async fn read_pipe_stdout(pipe: Option<tokio::process::ChildStdout>) -> Vec<u8> {
    use tokio::io::AsyncReadExt;
    match pipe {
        Some(mut p) => {
            let mut buf = Vec::new();
            let _ = p.read_to_end(&mut buf).await;
            buf
        }
        None => Vec::new(),
    }
}

/// Read all bytes from an optional child stderr pipe.
async fn read_pipe_stderr(pipe: Option<tokio::process::ChildStderr>) -> Vec<u8> {
    use tokio::io::AsyncReadExt;
    match pipe {
        Some(mut p) => {
            let mut buf = Vec::new();
            let _ = p.read_to_end(&mut buf).await;
            buf
        }
        None => Vec::new(),
    }
}

#[cfg(test)]
#[path = "subprocess.tests.rs"]
mod tests;
