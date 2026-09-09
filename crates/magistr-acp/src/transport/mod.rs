//! [`AcpTransport`] — the public handle on an agent session.

mod command;
mod command_loop;
mod connection;
mod handshake;
mod lines;
mod sender;
mod spawn;
mod wait_child;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::time::Duration;

use agent_client_protocol::schema::v1::{
    McpServer, Meta, SessionConfigOption, SessionConfigOptionValue, SessionInfo,
};
use agent_client_protocol::{Client, ConnectTo};
use tokio::io::AsyncBufReadExt;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use self::command::AcpCommand;
use self::connection::ConnectionInputs;
pub use self::sender::AcpSender;
use crate::handlers::terminal::{self, TerminalManager};
use crate::stderr_tail::StderrTail;
use crate::{AcpError, AcpEvent, AcpPermissionRequest, AcpSessionMode, Result};

/// How long `connect()` waits for the agent to finish its handshake. The fast
/// path never reaches this: a refusal, a crash or an early exit is reported
/// the moment the connection task sees it.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);

/// How long `close()` waits for the connection task to wind down.
const CLOSE_TIMEOUT: Duration = Duration::from_secs(5);

/// A session with one ACP agent.
///
/// `connect()` spawns the agent and completes the handshake; afterwards
/// `prompt()`, `cancel()`, `set_config_option()` and `list_sessions()` drive
/// the session, `read_events()` yields what the agent says, and
/// `take_permission_rx()` yields the permission prompts it asks for.
/// Dropping the transport kills the agent's whole process group.
pub struct AcpTransport {
    cli_path: PathBuf,
    cli_args: Vec<String>,
    cwd: Option<PathBuf>,
    session_mode: AcpSessionMode,
    mcp_servers: Vec<McpServer>,
    session_meta: Option<Meta>,
    ready: Arc<AtomicBool>,
    cmd_tx: Option<mpsc::UnboundedSender<AcpCommand>>,
    event_rx: Option<mpsc::UnboundedReceiver<Result<AcpEvent>>>,
    perm_rx: Option<mpsc::UnboundedReceiver<AcpPermissionRequest>>,
    connection: Option<JoinHandle<()>>,
    /// Process group of the agent (== its PID after `setpgid(0, 0)`); `0`
    /// before spawn.
    child_pgid: Arc<AtomicI32>,
    stderr_tail: StderrTail,
    /// The agent's terminals — the children `terminal/create` spawned,
    /// by id — shared with the connection so [`AcpTransport::kill_terminal`]
    /// reaches what the agent is running.
    terminals: TerminalManager,
}

impl AcpTransport {
    /// A transport for the agent at `cli_path`. The transport does not
    /// search for an agent itself: the caller resolves one (magistr's
    /// `AgentRuntime` does) and `None` is refused.
    pub fn new(cli_path: Option<PathBuf>, cwd: Option<PathBuf>) -> Result<Self> {
        let cli_path = cli_path.ok_or_else(|| {
            AcpError::CliNotFound(
                "no ACP agent named: pass the agent binary (magistr resolves one through \
                 AgentRuntime::detect)"
                    .to_string(),
            )
        })?;
        Self::new_with_args(cli_path, vec![], cwd)
    }

    /// A transport for an explicit binary and arguments (`["--acp"]` for
    /// Copilot), run in `cwd`.
    pub fn new_with_args(
        cli_path: PathBuf,
        cli_args: Vec<String>,
        cwd: Option<PathBuf>,
    ) -> Result<Self> {
        Ok(Self {
            cli_path,
            cli_args,
            cwd,
            session_mode: AcpSessionMode::New,
            mcp_servers: Vec::new(),
            session_meta: None,
            ready: Arc::new(AtomicBool::new(false)),
            cmd_tx: None,
            event_rx: None,
            perm_rx: None,
            connection: None,
            child_pgid: Arc::new(AtomicI32::new(0)),
            stderr_tail: StderrTail::default(),
            terminals: terminal::new_terminal_manager(),
        })
    }

    /// New session or resume. Set before `connect()`.
    #[must_use]
    pub fn session_mode(mut self, mode: AcpSessionMode) -> Self {
        self.session_mode = mode;
        self
    }

    /// MCP servers the agent should start for this session — typically the
    /// project's `.mcp.json` — sent on `session/new` / `session/load`. Set
    /// before `connect()`.
    #[must_use]
    pub fn mcp_servers(mut self, servers: Vec<McpServer>) -> Self {
        self.mcp_servers = servers;
        self
    }

    /// `_meta` to attach to `session/new` / `session/load` — agent-specific
    /// session options (an npm-era agent read SDK options from
    /// `claudeCode.options`); agents ignore keys they don't know. Set before
    /// `connect()`.
    #[must_use]
    pub fn session_meta(mut self, meta: Option<Meta>) -> Self {
        self.session_meta = meta;
        self
    }

    /// Spawn the agent and complete the handshake.
    pub async fn connect(&mut self) -> Result<()> {
        if self.is_ready() {
            return Ok(());
        }
        let command = spawn::build_command(&self.cli_path, &self.cli_args, self.cwd.as_deref());
        let spawned = spawn::spawn_agent(command)?;
        if let Some(pid) = spawned.child.id() {
            self.child_pgid.store(pid as i32, Ordering::SeqCst);
        }
        drain_stderr(spawned.stderr, self.stderr_tail.clone());
        let component = lines::agent_lines(spawned.stdin, spawned.stdout);
        self.start(component, Some(spawned.child)).await
    }

    /// Complete the handshake over an in-process component instead of a
    /// subprocess — an agent running on a [`Channel`](agent_client_protocol::Channel)
    /// under test, or embedded in the same binary.
    pub async fn connect_over(
        &mut self,
        component: impl ConnectTo<Client> + 'static,
    ) -> Result<()> {
        if self.is_ready() {
            return Ok(());
        }
        self.start(component, None).await
    }

    async fn start(
        &mut self,
        component: impl ConnectTo<Client> + 'static,
        child: Option<tokio::process::Child>,
    ) -> Result<()> {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (perm_tx, perm_rx) = mpsc::unbounded_channel();
        // The handshake outcome travels on its own channel so connect() never
        // drains event_rx — consumers expect the Initialized event there.
        let (handshake_tx, handshake_rx) = oneshot::channel();

        let cwd = match &self.cwd {
            Some(cwd) => cwd.clone(),
            None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        };
        let inputs = ConnectionInputs {
            child,
            cwd,
            session_mode: self.session_mode.clone(),
            mcp_servers: self.mcp_servers.clone(),
            session_meta: self.session_meta.clone(),
            cmd_rx,
            event_tx,
            perm_tx,
            ready: Arc::clone(&self.ready),
            handshake_tx,
            stderr_tail: self.stderr_tail.clone(),
            terminals: Arc::clone(&self.terminals),
        };
        let connection = tokio::spawn(connection::run_connection(component, inputs));

        match tokio::time::timeout(HANDSHAKE_TIMEOUT, handshake_rx).await {
            Ok(Ok(Ok(()))) => {}
            Ok(Ok(Err(e))) => {
                connection.abort();
                self.kill_child_process_group();
                return Err(e);
            }
            Ok(Err(_task_gone)) => {
                self.kill_child_process_group();
                return Err(AcpError::connection(self.stderr_tail.append_to(
                    "ACP connection task exited before the agent handshake completed".to_string(),
                )));
            }
            Err(_elapsed) => {
                connection.abort();
                self.kill_child_process_group();
                return Err(AcpError::timeout(self.stderr_tail.append_to(format!(
                    "ACP agent did not complete its handshake within {} seconds",
                    HANDSHAKE_TIMEOUT.as_secs()
                ))));
            }
        }

        self.cmd_tx = Some(cmd_tx);
        self.event_rx = Some(event_rx);
        self.perm_rx = Some(perm_rx);
        self.connection = Some(connection);
        Ok(())
    }

    /// Whether the handshake has completed and the session is live.
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::SeqCst)
    }

    /// The sending half, as a handle: the same command channel, usable
    /// after the transport itself has been handed to whoever runs the
    /// session.
    ///
    /// # Errors
    ///
    /// Before `connect()`.
    pub fn sender(&self) -> Result<AcpSender> {
        let cmd_tx = self.cmd_tx.as_ref().ok_or_else(|| {
            AcpError::transport("ACP transport not connected — cannot take a sender")
        })?;
        Ok(AcpSender::new(Arc::clone(&self.ready), cmd_tx.clone()))
    }

    fn send(&self, what: &str, command: AcpCommand) -> Result<()> {
        if !self.is_ready() {
            return Err(AcpError::transport(format!(
                "ACP transport is not ready — cannot {what}"
            )));
        }
        self.sender()
            .map_err(|_| {
                AcpError::transport(format!("ACP transport not connected — cannot {what}"))
            })?
            .send(what, command)
    }

    /// Send one user turn. The turn's updates and its `PromptResponse` arrive
    /// on the event channel.
    pub fn prompt(&self, text: &str) -> Result<()> {
        self.send("prompt", AcpCommand::Prompt(text.to_string()))
    }

    /// Ask the agent to stop the turn in flight. It finishes its current tool
    /// call and answers with `StopReason::Cancelled`; a no-op when idle.
    pub fn cancel(&self) -> Result<()> {
        self.send("cancel", AcpCommand::Cancel)
    }

    /// Kill the command on the agent's terminal `id` — the cockpit's
    /// held kill, done here exactly as the agent's own
    /// `terminal/kill` is: the child is signalled, the terminal stays
    /// valid, and the agent still reads its output and exit status
    /// and releases it. Nothing goes over the wire — the child is
    /// this process's.
    ///
    /// # Errors
    ///
    /// When no terminal `id` exists, or it was already released.
    pub fn kill_terminal(&self, id: &str) -> Result<()> {
        terminal::kill(&self.terminals, id)
            .map_err(|e| AcpError::transport(format!("cannot kill terminal {id}: {e}")))
    }

    /// Send a vendor extension request; see [`AcpSender::extension`].
    ///
    /// # Errors
    ///
    /// When the session is not live.
    pub fn extension(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<oneshot::Receiver<Result<serde_json::Value>>> {
        self.sender()?.extension(method, params)
    }

    /// `session/list`, optionally filtered by working directory.
    pub async fn list_sessions(&self, cwd: Option<PathBuf>) -> Result<Vec<SessionInfo>> {
        let (response_tx, response_rx) = oneshot::channel();
        self.send(
            "list sessions",
            AcpCommand::ListSessions { cwd, response_tx },
        )?;
        response_rx
            .await
            .map_err(|_| AcpError::transport("ACP connection dropped the list_sessions reply"))?
    }

    /// `session/set_config_option`. Returns the agent's full, updated option
    /// list — changing the model may change the available thought levels.
    pub async fn set_config_option(
        &self,
        session_id: String,
        config_id: String,
        value: SessionConfigOptionValue,
    ) -> Result<Vec<SessionConfigOption>> {
        let (response_tx, response_rx) = oneshot::channel();
        self.send(
            "set config option",
            AcpCommand::SetConfigOption {
                session_id,
                config_id,
                value,
                response_tx,
            },
        )?;
        response_rx.await.map_err(|_| {
            AcpError::transport("ACP connection dropped the set_config_option reply")
        })?
    }

    /// The event receiver — `Some` on the first call after `connect()`.
    pub fn read_events(&mut self) -> Option<mpsc::UnboundedReceiver<Result<AcpEvent>>> {
        self.event_rx.take()
    }

    /// A tap on the event stream: every `Ok` event, cloned, on a second
    /// receiver, while `read_events()` still yields the stream whole to
    /// whoever runs the session. `None` once `read_events()` has been
    /// taken — the tap must be spliced in first. The relay ends with
    /// either side.
    pub fn tap_events(&mut self) -> Option<mpsc::UnboundedReceiver<AcpEvent>> {
        let mut source = self.event_rx.take()?;
        let (forward_tx, forward_rx) = mpsc::unbounded_channel();
        let (tap_tx, tap_rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            while let Some(item) = source.recv().await {
                if let Ok(event) = &item {
                    let _ = tap_tx.send(event.clone());
                }
                if forward_tx.send(item).is_err() {
                    break;
                }
            }
        });
        self.event_rx = Some(forward_rx);
        Some(tap_rx)
    }

    /// The permission-request receiver — `Some` on the first call after
    /// `connect()`. Whoever takes it must answer (or drop) every request.
    pub fn take_permission_rx(&mut self) -> Option<mpsc::UnboundedReceiver<AcpPermissionRequest>> {
        self.perm_rx.take()
    }

    /// Process group of the agent, valid after `connect()`; `0` before, or
    /// when the session runs over an in-process component.
    pub fn child_pgid(&self) -> i32 {
        self.child_pgid.load(Ordering::SeqCst)
    }

    /// `SIGKILL` the agent's whole process group. Idempotent; no-op off Unix
    /// or before spawn.
    fn kill_child_process_group(&self) {
        #[cfg(unix)]
        {
            let pgid = self.child_pgid.load(Ordering::SeqCst);
            if pgid > 0 {
                // SAFETY: killpg is a plain syscall; a dead group returns ESRCH, which we ignore.
                unsafe {
                    libc::killpg(pgid, libc::SIGKILL);
                }
            }
        }
    }

    /// Cancel any turn in flight, let the connection wind down, and make sure
    /// the agent is gone.
    pub async fn close(&mut self) -> Result<()> {
        self.ready.store(false, Ordering::SeqCst);
        if let Some(tx) = self.cmd_tx.take() {
            let _ = tx.send(AcpCommand::Close);
        }
        if let Some(connection) = self.connection.take()
            && tokio::time::timeout(CLOSE_TIMEOUT, connection)
                .await
                .is_err()
        {
            tracing::warn!("ACP connection task did not finish in time; killing the agent");
        }
        self.kill_child_process_group();
        Ok(())
    }
}

impl Drop for AcpTransport {
    fn drop(&mut self) {
        self.ready.store(false, Ordering::SeqCst);
        // Kill first: works even when the task is wedged or we are unwinding.
        self.kill_child_process_group();
        if let Some(tx) = self.cmd_tx.take() {
            let _ = tx.send(AcpCommand::Close);
        }
        if let Some(connection) = self.connection.take() {
            connection.abort();
        }
    }
}

/// Keep the agent's last stderr lines (and drain the pipe so it never blocks).
fn drain_stderr(stderr: tokio::process::ChildStderr, tail: StderrTail) {
    tokio::spawn(async move {
        let mut lines = tokio::io::BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            tracing::debug!(target: "acp_agent_stderr", "{line}");
            tail.push(line);
        }
    });
}

#[cfg(test)]
#[path = "mod.tests.rs"]
mod tests;
