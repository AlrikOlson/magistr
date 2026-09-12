//! `terminal/*`: spawn commands for the agent and track their output.
//!
//! On Unix a command runs under a pseudo-terminal ([`magistr_process::Pty`]) so it sees a
//! tty; elsewhere — or when no pty can be had — it runs on pipes, and the
//! `terminal_create` event says which. Either way every chunk the command
//! writes streams to the consumer as [`AcpEvent::Terminal`] in order, and
//! the agent reads escape-stripped text through `terminal/output`.

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use agent_client_protocol::Error;
use agent_client_protocol::schema::v1::{
    CreateTerminalRequest, CreateTerminalResponse, KillTerminalRequest, KillTerminalResponse,
    ReleaseTerminalRequest, ReleaseTerminalResponse, TerminalExitStatus, TerminalOutputRequest,
    TerminalOutputResponse, WaitForTerminalExitRequest, WaitForTerminalExitResponse,
};
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;

use super::strip_escapes::EscapeStripper;
use crate::{AcpEvent, Result};

const INTERNAL_ERROR: i32 = -32603;
const INVALID_PARAMS: i32 = -32602;
/// How often `terminal/wait_for_exit` (and the pty reader) re-check a
/// running command.
const WAIT_POLL: Duration = Duration::from_millis(50);
/// How long `terminal/wait_for_exit` lets the readers drain after the
/// exit before answering anyway — a grandchild holding the stream open
/// must not hang the agent.
const EXIT_DRAIN_GRACE: Duration = Duration::from_secs(1);
/// The pty's size until a pane in the cockpit is there to measure.
#[cfg(unix)]
const PTY_SIZE: (u16, u16) = (80, 24);

/// One terminal the agent asked for.
pub(crate) struct TerminalState {
    child: tokio::process::Child,
    /// The escape-stripped text so far — what `terminal/output` answers.
    output: String,
    output_byte_limit: Option<u64>,
    truncated: bool,
    exit_status: Option<TerminalExitStatus>,
    released: bool,
    /// Whether we ended it. A killed terminal has no exit code to report:
    /// on Unix the signal leaves none, and on Windows `TerminateProcess`
    /// makes one up (1), which the agent must not read as the command's.
    killed: bool,
    /// Output readers still running: the output is whole once none are.
    readers: usize,
    stripper: EscapeStripper,
}

/// All live terminals, shared between the handlers and the output readers.
/// A `std::sync::Mutex` — never held across an `.await`.
pub(crate) type TerminalManager = Arc<Mutex<HashMap<String, TerminalState>>>;

type EventSender = mpsc::UnboundedSender<Result<AcpEvent>>;

pub(crate) fn new_terminal_manager() -> TerminalManager {
    Arc::new(Mutex::new(HashMap::new()))
}

fn lock(mgr: &TerminalManager) -> std::sync::MutexGuard<'_, HashMap<String, TerminalState>> {
    // A poisoned lock only means a reader task panicked mid-append; the map
    // itself is still usable.
    mgr.lock().unwrap_or_else(|p| p.into_inner())
}

fn unknown(tid: &str) -> Error {
    Error::new(INVALID_PARAMS, format!("Unknown terminal ID: {tid}"))
}

fn exit_status_of(status: std::process::ExitStatus, killed: bool) -> TerminalExitStatus {
    let code = if killed {
        None
    } else {
        status.code().map(|c| c as u32)
    };
    TerminalExitStatus::new().exit_code(code)
}

/// Unique-enough id for a terminal: nanos since the epoch plus a counter.
fn next_terminal_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("term-{ts:x}-{:x}", COUNTER.fetch_add(1, Ordering::Relaxed))
}

/// Tie a terminal's lifetime to ours on Linux; elsewhere `kill_on_drop` does.
#[cfg(target_os = "linux")]
fn configure_terminal_subprocess(cmd: &mut tokio::process::Command) {
    // SAFETY: `prctl` is async-signal-safe and runs between fork and exec.
    unsafe {
        cmd.pre_exec(|| {
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
            Ok(())
        });
    }
}

#[cfg(not(target_os = "linux"))]
fn configure_terminal_subprocess(_cmd: &mut tokio::process::Command) {}

/// How a spawned command's output is captured.
enum Capture {
    /// Under a pty: the master to read, and the parent's own slave handle
    /// that keeps macOS from flushing the tail on the child's exit.
    #[cfg(unix)]
    Pty {
        master: std::os::fd::OwnedFd,
        keepalive: std::os::fd::OwnedFd,
    },
    /// On pipes, with why no pty was had.
    Pipes {
        stdout: Option<tokio::process::ChildStdout>,
        stderr: Option<tokio::process::ChildStderr>,
        reason: String,
    },
}

impl Capture {
    const fn is_pty(&self) -> bool {
        match self {
            #[cfg(unix)]
            Self::Pty { .. } => true,
            Self::Pipes { .. } => false,
        }
    }

    fn reason(&self) -> Option<&str> {
        match self {
            #[cfg(unix)]
            Self::Pty { .. } => None,
            Self::Pipes { reason, .. } => Some(reason),
        }
    }
}

/// The command as asked, stdin closed: the pane is read-only, so a
/// command must never wait on input.
fn command(args: &CreateTerminalRequest) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(&args.command);
    cmd.args(&args.args);
    for var in &args.env {
        cmd.env(&var.name, &var.value);
    }
    if let Some(cwd) = &args.cwd {
        cmd.current_dir(cwd);
    }
    cmd.stdin(Stdio::null()).kill_on_drop(true);
    configure_terminal_subprocess(&mut cmd);
    cmd
}

fn spawn_error(args: &CreateTerminalRequest, e: &std::io::Error) -> Error {
    Error::new(
        INTERNAL_ERROR,
        format!("Failed to spawn terminal command '{}': {e}", args.command),
    )
}

#[cfg(unix)]
fn spawn_on_pty(
    args: &CreateTerminalRequest,
    pty: magistr_process::Pty,
) -> std::io::Result<(tokio::process::Child, Capture)> {
    let mut cmd = command(args);
    cmd.stdout(pty.stdio()?).stderr(pty.stdio()?);
    // SAFETY: `setsid` and the ioctl are raw syscalls, async-signal-safe
    // between fork and exec; fd 1 is the slave the parent just dup'd
    // there and stays open for the child's whole life.
    unsafe {
        cmd.pre_exec(|| {
            rustix::process::setsid()?;
            let stdout = std::os::fd::BorrowedFd::borrow_raw(1);
            rustix::process::ioctl_tiocsctty(stdout)?;
            Ok(())
        });
    }
    let child = cmd.spawn()?;
    let (master, keepalive) = pty.into_parts();
    Ok((child, Capture::Pty { master, keepalive }))
}

fn spawn_on_pipes(
    args: &CreateTerminalRequest,
    reason: String,
) -> std::io::Result<(tokio::process::Child, Capture)> {
    let mut cmd = command(args);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn()?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    Ok((
        child,
        Capture::Pipes {
            stdout,
            stderr,
            reason,
        },
    ))
}

/// A pty when the platform gives one, pipes otherwise.
fn spawn(args: &CreateTerminalRequest) -> std::io::Result<(tokio::process::Child, Capture)> {
    #[cfg(unix)]
    let reason = match magistr_process::Pty::open(PTY_SIZE.0, PTY_SIZE.1) {
        Ok(pty) => return spawn_on_pty(args, pty),
        Err(e) => format!("no pty: {e}"),
    };
    #[cfg(not(unix))]
    let reason = "no pty on this platform".to_owned();
    spawn_on_pipes(args, reason)
}

/// One chunk of output: streamed whole to the consumer, then stripped
/// into the agent's text under the byte limit.
fn ingest(mgr: &TerminalManager, tid: &str, event_tx: &EventSender, bytes: &[u8]) {
    let _ = event_tx.send(Ok(AcpEvent::Terminal {
        terminal_id: tid.to_owned(),
        bytes: bytes.to_vec(),
    }));
    let mut map = lock(mgr);
    if let Some(state) = map.get_mut(tid) {
        let text = state.stripper.feed(bytes);
        if state
            .output_byte_limit
            .is_some_and(|limit| state.output.len() as u64 >= limit)
        {
            state.truncated = true;
            return;
        }
        state.output.push_str(&text);
    }
}

fn reader_done(mgr: &TerminalManager, tid: &str) {
    if let Some(state) = lock(mgr).get_mut(tid) {
        state.readers = state.readers.saturating_sub(1);
    }
}

/// Whether the child is gone — reaping it and recording the status when
/// it just exited. An unknown terminal counts as gone.
#[cfg(unix)]
fn child_exited(mgr: &TerminalManager, tid: &str) -> bool {
    let mut map = lock(mgr);
    let Some(state) = map.get_mut(tid) else {
        return true;
    };
    if state.exit_status.is_some() {
        return true;
    }
    match state.child.try_wait() {
        Ok(Some(status)) => {
            state.exit_status = Some(exit_status_of(status, state.killed));
            true
        }
        Ok(None) => false,
        Err(_) => true,
    }
}

/// Copy one of the child's pipes into the terminal's output.
fn spawn_pipe_reader<R>(mgr: TerminalManager, tid: String, event_tx: EventSender, mut reader: R)
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => ingest(&mgr, &tid, &event_tx, &buf[..n]),
            }
        }
        reader_done(&mgr, &tid);
    });
}

/// Read the pty master until the child has exited and the master is
/// drained; only then does the keepalive slave close, so nothing the
/// child wrote is lost on either kernel.
#[cfg(unix)]
fn spawn_pty_reader(
    mgr: TerminalManager,
    tid: String,
    event_tx: EventSender,
    master: std::os::fd::OwnedFd,
    keepalive: std::os::fd::OwnedFd,
) {
    use std::io::Read;

    use tokio::io::Interest;
    use tokio::io::unix::AsyncFd;

    tokio::spawn(async move {
        let _keepalive = keepalive;
        let master = std::fs::File::from(master);
        if let Ok(fd) = AsyncFd::with_interest(master, Interest::READABLE) {
            let mut buf = [0u8; 4096];
            loop {
                tokio::select! {
                    guard = fd.readable() => {
                        let Ok(mut guard) = guard else { break };
                        match guard.try_io(|inner| inner.get_ref().read(&mut buf)) {
                            Ok(Ok(0)) | Ok(Err(_)) => break,
                            Ok(Ok(n)) => ingest(&mgr, &tid, &event_tx, &buf[..n]),
                            Err(_would_block) => {}
                        }
                    }
                    () = tokio::time::sleep(WAIT_POLL) => {
                        if child_exited(&mgr, &tid) {
                            loop {
                                match fd.get_ref().read(&mut buf) {
                                    Ok(0) | Err(_) => break,
                                    Ok(n) => ingest(&mgr, &tid, &event_tx, &buf[..n]),
                                }
                            }
                            break;
                        }
                    }
                }
            }
        }
        reader_done(&mgr, &tid);
    });
}

pub(crate) fn create_terminal(
    args: CreateTerminalRequest,
    mgr: &TerminalManager,
    event_tx: &EventSender,
) -> std::result::Result<CreateTerminalResponse, Error> {
    let terminal_id = next_terminal_id();
    let (child, capture) = spawn(&args).map_err(|e| spawn_error(&args, &e))?;
    let pty = capture.is_pty();
    let reason = capture.reason().map(str::to_owned);
    let readers = match &capture {
        #[cfg(unix)]
        Capture::Pty { .. } => 1,
        Capture::Pipes { stdout, stderr, .. } => {
            usize::from(stdout.is_some()) + usize::from(stderr.is_some())
        }
    };

    lock(mgr).insert(
        terminal_id.clone(),
        TerminalState {
            child,
            output: String::new(),
            output_byte_limit: args.output_byte_limit,
            truncated: false,
            exit_status: None,
            released: false,
            killed: false,
            readers,
            stripper: EscapeStripper::default(),
        },
    );
    match capture {
        #[cfg(unix)]
        Capture::Pty { master, keepalive } => spawn_pty_reader(
            Arc::clone(mgr),
            terminal_id.clone(),
            event_tx.clone(),
            master,
            keepalive,
        ),
        Capture::Pipes { stdout, stderr, .. } => {
            if let Some(stdout) = stdout {
                spawn_pipe_reader(
                    Arc::clone(mgr),
                    terminal_id.clone(),
                    event_tx.clone(),
                    stdout,
                );
            }
            if let Some(stderr) = stderr {
                spawn_pipe_reader(
                    Arc::clone(mgr),
                    terminal_id.clone(),
                    event_tx.clone(),
                    stderr,
                );
            }
        }
    }

    let _ = event_tx.send(Ok(AcpEvent::System {
        subtype: "terminal_create".to_string(),
        data: serde_json::json!({
            "terminal_id": &terminal_id,
            "command": &args.command,
            "args": &args.args,
            "pty": pty,
            "pane": if pty { "live" } else { "text-only" },
            "reason": reason,
        }),
    }));
    Ok(CreateTerminalResponse::new(terminal_id))
}

pub(crate) fn terminal_output(
    args: &TerminalOutputRequest,
    mgr: &TerminalManager,
) -> std::result::Result<TerminalOutputResponse, Error> {
    let tid = args.terminal_id.0.to_string();
    let map = lock(mgr);
    let state = map.get(&tid).ok_or_else(|| unknown(&tid))?;
    Ok(TerminalOutputResponse::new(&state.output, state.truncated)
        .exit_status(state.exit_status.clone()))
}

/// Poll until the command exits and its output is drained (or the grace
/// after the exit runs out). Long-running by design — the caller spawns it.
pub(crate) async fn wait_for_terminal_exit(
    args: &WaitForTerminalExitRequest,
    mgr: &TerminalManager,
) -> std::result::Result<WaitForTerminalExitResponse, Error> {
    let tid = args.terminal_id.0.to_string();
    let mut drain_deadline: Option<Instant> = None;
    loop {
        let exited = {
            let mut map = lock(mgr);
            let state = map.get_mut(&tid).ok_or_else(|| unknown(&tid))?;
            if state.exit_status.is_none() {
                match state.child.try_wait() {
                    Ok(Some(status)) => {
                        state.exit_status = Some(exit_status_of(status, state.killed));
                    }
                    Ok(None) => {}
                    Err(e) => {
                        return Err(Error::new(
                            INTERNAL_ERROR,
                            format!("Failed to wait for terminal {tid}: {e}"),
                        ));
                    }
                }
            }
            state.exit_status.clone().map(|exit| (exit, state.readers))
        };
        match exited {
            Some((exit, 0)) => return Ok(WaitForTerminalExitResponse::new(exit)),
            Some((exit, _)) => {
                let deadline =
                    *drain_deadline.get_or_insert_with(|| Instant::now() + EXIT_DRAIN_GRACE);
                if Instant::now() >= deadline {
                    return Ok(WaitForTerminalExitResponse::new(exit));
                }
            }
            None => {}
        }
        tokio::time::sleep(WAIT_POLL).await;
    }
}

pub(crate) fn kill_terminal(
    args: &KillTerminalRequest,
    mgr: &TerminalManager,
) -> std::result::Result<KillTerminalResponse, Error> {
    kill(mgr, &args.terminal_id.0)?;
    Ok(KillTerminalResponse::new())
}

/// Kill terminal `tid`'s command without releasing the terminal — what
/// the agent's `terminal/kill` does, and what the cockpit's own held
/// kill does through the transport handle: the id
/// stays valid, so the agent still reads the output and the exit
/// status and releases it when it is done.
pub(crate) fn kill(mgr: &TerminalManager, tid: &str) -> std::result::Result<(), Error> {
    let mut map = lock(mgr);
    let state = map.get_mut(tid).ok_or_else(|| unknown(tid))?;
    if state.released {
        return Err(Error::new(
            INVALID_PARAMS,
            format!("Terminal {tid} has been released"),
        ));
    }
    state.killed = true;
    let _ = state.child.start_kill();
    if state.exit_status.is_none()
        && let Ok(Some(status)) = state.child.try_wait()
    {
        state.exit_status = Some(exit_status_of(status, true));
    }
    Ok(())
}

pub(crate) fn release_terminal(
    args: &ReleaseTerminalRequest,
    mgr: &TerminalManager,
    event_tx: &EventSender,
) -> std::result::Result<ReleaseTerminalResponse, Error> {
    let tid = args.terminal_id.0.to_string();
    {
        let mut map = lock(mgr);
        let state = map.get_mut(&tid).ok_or_else(|| unknown(&tid))?;
        if state.exit_status.is_none() {
            state.killed = true;
            let _ = state.child.start_kill();
            if let Ok(Some(status)) = state.child.try_wait() {
                state.exit_status = Some(exit_status_of(status, true));
            }
        }
        state.released = true;
    }
    let _ = event_tx.send(Ok(AcpEvent::System {
        subtype: "terminal_release".to_string(),
        data: serde_json::json!({ "terminal_id": &tid }),
    }));
    Ok(ReleaseTerminalResponse::new())
}

#[cfg(test)]
#[path = "terminal.tests.rs"]
mod tests;
