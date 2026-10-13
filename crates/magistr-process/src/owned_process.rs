use std::io;
use std::process::Stdio;
use std::time::{Duration, Instant};

use tokio::io::AsyncReadExt;
use tokio::process::{Child, ChildStderr, ChildStdout, Command};
use tokio_util::sync::CancellationToken;

#[cfg(unix)]
use crate::OutputMode;
use crate::{BoundedCapture, Launch, ProcessGroup, ProcessOutcome, ProcessSpec, Termination};

/// How long, once the group is dead or exited, the pipes get to close
/// before what is left in them is abandoned.
const DRAIN_AFTER_END: Duration = Duration::from_secs(2);

/// How often the pty reader asks whether the child has exited.
#[cfg(unix)]
const WAIT_POLL: Duration = Duration::from_millis(50);

/// The child's output, as spawned.
enum Streams {
    Pipes {
        stdout: Option<ChildStdout>,
        stderr: Option<ChildStderr>,
    },
    #[cfg(unix)]
    Terminal {
        master: std::os::fd::OwnedFd,
        /// The parent's own slave: macOS discards a slave's unread output
        /// when its last descriptor closes, so this one lives until the
        /// master has been drained after the exit.
        keepalive: std::os::fd::OwnedFd,
    },
}

/// One spawned child in its own session and process group. [`Self::run`]
/// consumes it and answers a [`ProcessOutcome`]; dropping it before that
/// kills its process group and retains child reaping ownership. Registration
/// follows the group through timeout, cancellation, panic and caller drop.
pub struct OwnedProcess {
    child: crate::ReapingChild,
    group: Option<ProcessGroup>,
    pid: Option<u32>,
    streams: Streams,
    timeout: Duration,
    capture_bytes: usize,
    termination: Termination,
    started: Instant,
}

impl OwnedProcess {
    /// Spawn `spec`'s launch with stdin closed and both streams captured.
    ///
    /// # Errors
    /// The program could not be started (not found, refused, an empty
    /// argv): nothing ran.
    pub fn spawn(spec: ProcessSpec) -> io::Result<Self> {
        let mut command = command_for(&spec)?;
        if let Some(cwd) = &spec.cwd {
            command.current_dir(cwd);
        }
        command.stdin(Stdio::null()).kill_on_drop(true);
        #[cfg(unix)]
        let pty = match spec.output {
            OutputMode::Terminal { cols, rows } => crate::Pty::open(cols, rows).ok(),
            OutputMode::Pipes => None,
        };
        #[cfg(not(unix))]
        let _ = spec.output;
        #[cfg(unix)]
        let terminal = pty.is_some();
        #[cfg(unix)]
        match &pty {
            Some(pty) => {
                command.stdout(pty.stdio()?).stderr(pty.stdio()?);
            }
            None => {
                command.stdout(Stdio::piped()).stderr(Stdio::piped());
            }
        }
        #[cfg(not(unix))]
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        // SAFETY: `setsid` and the ioctl are raw syscalls, async-signal-safe
        // between fork and exec; on a pty, fd 1 is the slave the parent just
        // dup'd there and it stays open for the child's whole life.
        #[cfg(unix)]
        unsafe {
            command.pre_exec(move || {
                if libc::setsid() < 0 {
                    return Err(io::Error::last_os_error());
                }
                if terminal && libc::ioctl(1, libc::TIOCSCTTY as _, 0) < 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let started = Instant::now();
        #[cfg(not(windows))]
        let mut child = crate::ReapingChild::new(command.spawn()?);
        #[cfg(windows)]
        let (mut child, owned_group) = ProcessGroup::spawn(&mut command)?;
        let pid = child.id();
        // SAFETY: pre_exec created an isolated session/group for this child.
        #[cfg(not(windows))]
        let group = pid.map(|pid| unsafe { ProcessGroup::from_spawned_pid(pid) });
        #[cfg(windows)]
        let group = Some(owned_group);
        #[cfg(unix)]
        let streams = match pty {
            Some(pty) => {
                let (master, keepalive) = pty.into_parts();
                Streams::Terminal { master, keepalive }
            }
            None => Streams::Pipes {
                stdout: child.stdout.take(),
                stderr: child.stderr.take(),
            },
        };
        #[cfg(not(unix))]
        let streams = Streams::Pipes {
            stdout: child.stdout.take(),
            stderr: child.stderr.take(),
        };
        Ok(Self {
            child,
            group,
            pid,
            streams,
            timeout: spec.timeout,
            capture_bytes: spec.capture_bytes,
            termination: spec.termination,
            started,
        })
    }

    /// The spawned process id (its group id on Unix).
    #[must_use]
    pub fn pid(&self) -> Option<u32> {
        self.pid
    }

    /// Whether the child got the pty it asked for.
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        match self.streams {
            Streams::Pipes { .. } => false,
            #[cfg(unix)]
            Streams::Terminal { .. } => true,
        }
    }

    /// Read until the child has exited and its output is drained, racing
    /// the deadline and `cancel`; on either, the whole group is ended the
    /// way the spec's termination says. Every chunk reaches `sink` as it
    /// lands; the answer carries the bounded capture of both streams.
    pub async fn run(
        self,
        cancel: &CancellationToken,
        mut sink: impl FnMut(&[u8]) + Send,
    ) -> ProcessOutcome {
        let Self {
            mut child,
            mut group,
            pid,
            streams,
            timeout,
            capture_bytes,
            termination,
            started,
        } = self;
        let mut outcome = ProcessOutcome {
            pid,
            ..ProcessOutcome::default()
        };
        let mut stdout = BoundedCapture::new(capture_bytes);
        let mut stderr = BoundedCapture::new(capture_bytes);
        let deadline = tokio::time::sleep(timeout);
        tokio::pin!(deadline);
        let status = match streams {
            Streams::Pipes {
                stdout: out,
                stderr: err,
            } => {
                let readers = read_pipes(out, err, &mut stdout, &mut stderr, &mut sink);
                tokio::pin!(readers);
                let mut status = None;
                let mut drained = false;
                loop {
                    tokio::select! {
                        result = child.wait(), if status.is_none() => {
                            status = Some(result);
                            if drained { break; }
                        }
                        () = &mut readers, if !drained => {
                            drained = true;
                            if status.is_some() { break; }
                        }
                        () = &mut deadline => { outcome.timed_out = true; break; }
                        () = cancel.cancelled() => { outcome.cancelled = true; break; }
                    }
                }
                outcome.record_cleanup(end_group(&mut child, group.as_mut(), termination).await);
                // Bound the drain even if settlement failed or a descriptor
                // escaped the group; a stray descriptor cannot hang the caller.
                if !drained {
                    let _ = tokio::time::timeout(DRAIN_AFTER_END, &mut readers).await;
                }
                status
            }
            #[cfg(unix)]
            Streams::Terminal { master, keepalive } => {
                let status = read_terminal(
                    &mut child,
                    master,
                    &mut stdout,
                    &mut sink,
                    &mut deadline,
                    cancel,
                    &mut outcome,
                )
                .await;
                outcome.record_cleanup(end_group(&mut child, group.as_mut(), termination).await);
                // Settlement was attempted and any failure recorded. Release
                // the parent's slave without extending a failed cleanup.
                drop(keepalive);
                status
            }
        };
        match status {
            Some(Ok(status)) => {
                outcome.exit_code = status.code();
                #[cfg(unix)]
                {
                    use std::os::unix::process::ExitStatusExt;
                    outcome.signal = status.signal();
                }
            }
            Some(Err(error)) => outcome.fault = Some(format!("wait: {error}")),
            None => {}
        }
        outcome.duration = started.elapsed();
        outcome.stdout = stdout.text();
        outcome.stderr = stderr.text();
        outcome.dropped_bytes = stdout.dropped() + stderr.dropped();
        outcome
    }
}

/// The platform command for a launch with the spec's environment; an
/// empty argv is refused before anything runs.
#[cfg(not(windows))]
fn command_for(spec: &ProcessSpec) -> io::Result<Command> {
    let mut command = match &spec.launch {
        Launch::Shell(line) => {
            let mut command = Command::new("sh");
            command.arg("-c").arg(line);
            command
        }
        Launch::Argv(argv) => {
            let (program, args) = split_argv(argv)?;
            let mut command = Command::new(program);
            command.args(args);
            command
        }
    };
    for (name, value) in &spec.env {
        command.env(name, value);
    }
    Ok(command)
}

/// The platform command for a launch with the spec's environment. A line
/// is written in POSIX shell — every declared command, gate and hook in
/// this workspace is — so it runs under the POSIX shell found on PATH (Git
/// for Windows'), with that shell's directory first on the child's PATH:
/// a nested `bash` or an `env bash` shebang then resolves beside it and
/// never to a WSL launcher. A bare `bash` or `sh` argv runs that shell the
/// same way. Without a POSIX shell a line runs under `cmd`, except one
/// that starts with `bash` or `sh`, which is refused rather than handed to
/// WSL; so is a bare `bash` or `sh` argv.
#[cfg(windows)]
fn command_for(spec: &ProcessSpec) -> io::Result<Command> {
    use crate::posix_shell::{PosixShell, line_starts_with_posix_shell, names_posix_shell};

    let path = spec
        .env
        .iter()
        .rev()
        .find(|(name, _)| name.eq_ignore_ascii_case("PATH"))
        .map(|(_, value)| std::ffi::OsString::from(value))
        .or_else(|| std::env::var_os("PATH"));
    let system_root = std::env::var_os("SystemRoot");
    let shell = PosixShell::locate(path.as_deref(), system_root.as_deref());
    let refused = |what: &str| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "no POSIX shell on PATH (install Git for Windows); {what} was not run,                  because `bash` here would start WSL"
            ),
        )
    };
    let (mut command, runs_shell) = match &spec.launch {
        Launch::Shell(line) => match &shell {
            Some(shell) => {
                let mut command = Command::new(shell.sh());
                command.arg("-c").arg(line);
                (command, true)
            }
            None if line_starts_with_posix_shell(line) => {
                return Err(refused(&format!("`{line}`")));
            }
            None => {
                let mut command = Command::new("cmd");
                command.arg("/C").arg(line);
                (command, false)
            }
        },
        Launch::Argv(argv) => {
            let (program, args) = split_argv(argv)?;
            if names_posix_shell(program) {
                let shell = shell
                    .as_ref()
                    .ok_or_else(|| refused(&format!("`{program}`")))?;
                let program = if program.to_ascii_lowercase().starts_with("bash") {
                    shell.bash()
                } else {
                    shell.sh()
                };
                let mut command = Command::new(program);
                command.args(args);
                (command, true)
            } else {
                let mut command = Command::new(program);
                command.args(args);
                (command, false)
            }
        }
    };
    for (name, value) in &spec.env {
        command.env(name, value);
    }
    if let (true, Some(shell)) = (runs_shell, &shell) {
        command.env("PATH", shell.path_for_child(path.as_deref()));
    }
    Ok(command)
}

/// The program and its arguments; an empty argv starts nothing.
fn split_argv(argv: &[String]) -> io::Result<(&String, &[String])> {
    argv.split_first()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "empty argv: nothing to start"))
}

/// Both pipes until each has closed, every chunk to the sink and its
/// capture in arrival order.
async fn read_pipes(
    mut out: Option<ChildStdout>,
    mut err: Option<ChildStderr>,
    stdout: &mut BoundedCapture,
    stderr: &mut BoundedCapture,
    sink: &mut (impl FnMut(&[u8]) + Send),
) {
    let mut out_buf = vec![0_u8; 16 * 1024];
    let mut err_buf = vec![0_u8; 16 * 1024];
    loop {
        tokio::select! {
            read = read_some(out.as_mut(), &mut out_buf), if out.is_some() => match read {
                Ok(count) if count > 0 => { sink(&out_buf[..count]); stdout.push(&out_buf[..count]); }
                _ => out = None,
            },
            read = read_some(err.as_mut(), &mut err_buf), if err.is_some() => match read {
                Ok(count) if count > 0 => { sink(&err_buf[..count]); stderr.push(&err_buf[..count]); }
                _ => err = None,
            },
            else => break,
        }
    }
}

/// One read from an open stream; an absent one never answers (its branch
/// is disabled by the precondition, this only keeps the expression total).
async fn read_some<R: tokio::io::AsyncRead + Unpin>(
    stream: Option<&mut R>,
    buf: &mut [u8],
) -> io::Result<usize> {
    match stream {
        Some(stream) => stream.read(buf).await,
        None => std::future::pending().await,
    }
}

/// The pty master until the child has exited and the master is drained,
/// racing the deadline and the cancel. A master never signals EOF while
/// the parent holds the slave, so the exit is polled.
#[cfg(unix)]
async fn read_terminal(
    child: &mut Child,
    master: std::os::fd::OwnedFd,
    stdout: &mut BoundedCapture,
    sink: &mut (impl FnMut(&[u8]) + Send),
    deadline: &mut std::pin::Pin<&mut tokio::time::Sleep>,
    cancel: &CancellationToken,
    outcome: &mut ProcessOutcome,
) -> Option<io::Result<std::process::ExitStatus>> {
    use std::io::Read;

    use tokio::io::Interest;
    use tokio::io::unix::AsyncFd;

    let master = match AsyncFd::with_interest(std::fs::File::from(master), Interest::READABLE) {
        Ok(master) => master,
        Err(error) => {
            outcome.fault = Some(format!("pty master: {error}"));
            return None;
        }
    };
    let mut buf = [0_u8; 4096];
    let work = async {
        loop {
            tokio::select! {
                guard = master.readable() => {
                    let mut guard = guard?;
                    match guard.try_io(|inner| inner.get_ref().read(&mut buf)) {
                        Ok(Ok(0)) | Ok(Err(_)) => break,
                        Ok(Ok(count)) => { sink(&buf[..count]); stdout.push(&buf[..count]); }
                        Err(_would_block) => {}
                    }
                }
                () = tokio::time::sleep(WAIT_POLL) => {
                    if let Some(status) = child.try_wait()? {
                        loop {
                            match master.get_ref().read(&mut buf) {
                                Ok(count) if count > 0 => { sink(&buf[..count]); stdout.push(&buf[..count]); }
                                _ => break,
                            }
                        }
                        return Ok(status);
                    }
                }
            }
        }
        child.wait().await
    };
    tokio::pin!(work);
    tokio::select! {
        status = &mut work => Some(status),
        () = deadline => { outcome.timed_out = true; None }
        () = cancel.cancelled() => { outcome.cancelled = true; None }
    }
}

/// End the group this process leads — its id is the child's own pid,
/// never a caller-supplied value — then reap the leader.
async fn end_group(
    child: &mut Child,
    mut group: Option<&mut ProcessGroup>,
    termination: Termination,
) -> io::Result<()> {
    if child.try_wait()?.is_some()
        && group
            .as_mut()
            .map_or(Ok(true), |group| group.is_settled())?
    {
        return Ok(());
    }
    #[cfg(unix)]
    if let Termination::Graceful { grace } = termination {
        if let Some(group) = group.as_ref() {
            let _ = group.signal(libc::SIGTERM);
        }
        if let Ok(result) =
            tokio::time::timeout(grace, reap_group(child, group.as_deref_mut())).await
        {
            return result;
        }
    }
    #[cfg(not(unix))]
    let _ = termination;
    if let Some(group) = group.as_ref() {
        let _ = group.kill();
    }
    // Signal delivery can race exit; only the bounded reap/probe below
    // establishes settlement. A persistent refusal becomes a cleanup fault.
    let _ = child.start_kill();
    tokio::time::timeout(
        crate::process_group::CLEANUP_TIMEOUT,
        reap_group(child, group),
    )
    .await
    .map_err(|_| {
        io::Error::new(
            io::ErrorKind::TimedOut,
            "process group did not settle by cleanup deadline",
        )
    })?
}

async fn reap_group(child: &mut Child, mut group: Option<&mut ProcessGroup>) -> io::Result<()> {
    loop {
        let reaped = child.try_wait()?.is_some();
        if reaped
            && group
                .as_mut()
                .map_or(Ok(true), |group| group.is_settled())?
        {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[cfg(test)]
#[path = "owned_process.tests.rs"]
mod tests;

#[cfg(all(test, windows))]
#[path = "owned_process_windows.tests.rs"]
mod windows_tests;
