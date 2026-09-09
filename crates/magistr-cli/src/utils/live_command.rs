//! Run a shell command under a pseudo-terminal and stream what it writes.
//!
//! [`run_command_live`] is the runner behind every quality gate. The
//! command sees a tty — colours, progress bars, its own width — every
//! chunk it writes reaches the caller's sink raw and in order while it
//! runs, and the result carries the escape-stripped transcript the trace
//! keeps. Where no pty can be had (a platform without one, a master that
//! will not open, an empty command) it falls back to the pipe runner and
//! hands the sink the captured text once at the end: a text-only pane
//! rather than a blank one.

use std::path::Path;

use tokio_util::sync::CancellationToken;

use super::subprocess::{CommandResult, run_command_cancellable};

/// The PTY's fixed size. Its 140 columns match rustc's default diagnostic
/// width outside a terminal, keeping recorded diagnostics consistently wrapped.
#[cfg(unix)]
const PTY_SIZE: (u16, u16) = (140, 40);

/// Run `sh -c command` in `dir` on a pty, streaming every chunk it writes
/// to `sink` as it lands and answering with the stripped transcript in
/// `stdout` (`stderr` stays empty: a tty is one stream). The timeout, the
/// cancel and the process-group cleanup are
/// [`run_command_cancellable`]'s, and so are the results they answer
/// with; the pipe runner itself is the fallback where a pty is not to be
/// had, its captured text given to the sink once at the end.
pub async fn run_command_live(
    command: &str,
    dir: &Path,
    timeout_secs: u64,
    cancel: &CancellationToken,
    mut sink: impl FnMut(&[u8]) + Send,
) -> CommandResult {
    #[cfg(unix)]
    if !command.is_empty() {
        match pty::spawn(command, dir) {
            Ok(live) => return pty::run(live, timeout_secs, cancel, &mut sink).await,
            Err(e) => tracing::debug!(
                target: "magistr::gate",
                "no pty for the command, falling back to pipes: {e}"
            ),
        }
    }
    let result = run_command_cancellable(command, dir, timeout_secs, None, cancel).await;
    for text in [&result.stdout, &result.stderr] {
        if !text.is_empty() {
            sink(text.as_bytes());
        }
    }
    result
}

#[cfg(unix)]
mod pty {
    use std::fs::File;
    use std::io::Read;
    use std::os::fd::OwnedFd;
    use std::path::Path;
    use std::process::{ExitStatus, Stdio};
    use std::time::{Duration, Instant};

    use magistr_acp::{EscapeStripper, Pty};
    use tokio::io::Interest;
    use tokio::io::unix::AsyncFd;
    use tokio::process::{Child, Command};
    use tokio_util::sync::CancellationToken;

    use crate::utils::child_guard;
    use crate::utils::subprocess::{CommandResult, graceful_kill};

    /// How often the reader asks whether the child has exited.
    const WAIT_POLL: Duration = Duration::from_millis(50);

    /// A spawned command and the pty it writes to.
    pub(super) struct Live {
        child: Child,
        master: OwnedFd,
        /// The parent's own slave: macOS discards a slave's unread output
        /// when its last descriptor closes, so this one lives until the
        /// master has been drained after the exit.
        keepalive: OwnedFd,
        pgid: i32,
        started: Instant,
    }

    /// What the race between the child, the clock and the caller decided.
    enum Waited {
        Finished(std::io::Result<ExitStatus>),
        TimedOut,
        Cancelled,
    }

    /// Spawn `sh -c command` in `dir` with both streams on a fresh pty's
    /// slave, in its own session with the pty as its controlling
    /// terminal, registered with the child guard.
    pub(super) fn spawn(command: &str, dir: &Path) -> std::io::Result<Live> {
        let pty = Pty::open(super::PTY_SIZE.0, super::PTY_SIZE.1)?;
        let mut cmd = Command::new("sh");
        cmd.arg("-c")
            .arg(command)
            .current_dir(dir)
            .stdin(Stdio::null())
            .stdout(pty.stdio()?)
            .stderr(pty.stdio()?);
        // SAFETY: `setsid` and the ioctl are raw syscalls, async-signal-safe
        // between fork and exec; fd 1 is the slave the parent just dup'd
        // there and stays open for the child's whole life.
        unsafe {
            cmd.pre_exec(|| {
                if libc::setsid() < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::ioctl(1, libc::TIOCSCTTY as _, 0) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let started = Instant::now();
        let child = cmd.spawn()?;
        let pgid = child.id().map_or(0, |pid| pid as i32);
        child_guard::register_child_pgid(pgid);
        let (master, keepalive) = pty.into_parts();
        Ok(Live {
            child,
            master,
            keepalive,
            pgid,
            started,
        })
    }

    /// Read the master until the child has exited and the master is
    /// drained, racing the clock and `cancel` exactly as the pipe runner
    /// does; the process group is ended the same way on either.
    pub(super) async fn run<S: FnMut(&[u8]) + Send>(
        live: Live,
        timeout_secs: u64,
        cancel: &CancellationToken,
        sink: &mut S,
    ) -> CommandResult {
        let Live {
            mut child,
            master,
            keepalive,
            pgid,
            started,
        } = live;
        let master = AsyncFd::with_interest(File::from(master), Interest::READABLE);
        let mut stripper = EscapeStripper::default();
        let mut text = String::new();
        let waited = match &master {
            Ok(master) => {
                let work = read_until_exit(&mut child, master, &mut stripper, &mut text, sink);
                tokio::pin!(work);
                tokio::select! {
                    result = tokio::time::timeout(Duration::from_secs(timeout_secs), &mut work) => {
                        match result {
                            Ok(status) => Waited::Finished(status),
                            Err(_) => Waited::TimedOut,
                        }
                    }
                    () = cancel.cancelled() => Waited::Cancelled,
                }
            }
            Err(e) => Waited::Finished(Err(std::io::Error::new(e.kind(), e.to_string()))),
        };
        let duration = started.elapsed();
        let result = match waited {
            Waited::Finished(Ok(status)) => CommandResult {
                exit_code: status.code().unwrap_or(-1),
                stdout: text,
                stderr: String::new(),
                duration,
                timed_out: false,
                cancelled: false,
            },
            Waited::Finished(Err(e)) => {
                graceful_kill(&mut child, pgid).await;
                CommandResult {
                    exit_code: -1,
                    stdout: String::new(),
                    stderr: format!("failed to execute: {e}"),
                    duration,
                    timed_out: false,
                    cancelled: false,
                }
            }
            Waited::TimedOut => {
                graceful_kill(&mut child, pgid).await;
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
                graceful_kill(&mut child, pgid).await;
                CommandResult {
                    exit_code: -1,
                    stdout: String::new(),
                    stderr: "cancelled".into(),
                    duration,
                    timed_out: false,
                    cancelled: true,
                }
            }
        };
        child_guard::unregister_child_pgid(pgid);
        // The child is gone either way: only now may the parent's slave
        // close, and the master with it.
        drop(keepalive);
        drop(master);
        result
    }

    /// Every chunk until the child has exited and the master is drained.
    async fn read_until_exit<S: FnMut(&[u8]) + Send>(
        child: &mut Child,
        master: &AsyncFd<File>,
        stripper: &mut EscapeStripper,
        text: &mut String,
        sink: &mut S,
    ) -> std::io::Result<ExitStatus> {
        let mut buf = [0u8; 4096];
        loop {
            tokio::select! {
                guard = master.readable() => {
                    let mut guard = guard?;
                    match guard.try_io(|inner| inner.get_ref().read(&mut buf)) {
                        Ok(Ok(0)) | Ok(Err(_)) => break,
                        Ok(Ok(n)) => ingest(&buf[..n], stripper, text, sink),
                        Err(_would_block) => {}
                    }
                }
                () = tokio::time::sleep(WAIT_POLL) => {
                    if let Some(status) = child.try_wait()? {
                        loop {
                            match master.get_ref().read(&mut buf) {
                                Ok(n) if n > 0 => ingest(&buf[..n], stripper, text, sink),
                                _ => break,
                            }
                        }
                        return Ok(status);
                    }
                }
            }
        }
        child.wait().await
    }

    /// One chunk: to the sink whole, and stripped into the transcript.
    fn ingest<S: FnMut(&[u8]) + Send>(
        chunk: &[u8],
        stripper: &mut EscapeStripper,
        text: &mut String,
        sink: &mut S,
    ) {
        sink(chunk);
        text.push_str(&stripper.feed(chunk));
    }
}

#[cfg(test)]
#[path = "live_command.tests.rs"]
mod tests;
