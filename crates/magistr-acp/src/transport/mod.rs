//! [`AcpTransport`] — the public handle on an agent session.

mod command;
mod command_loop;
mod connection;
mod extension_reply;
mod handshake;
mod lines;
mod prompt_content;
mod sender;
mod session_choices;
mod spawn;
mod wait_child;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
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
use magistr_application::effects::{EffectPort, SourceRoute};

use crate::capabilities::LocalEffectPort;
use crate::handlers::effects::EffectBridge;
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
    background_tasks: Vec<JoinHandle<()>>,
    /// Owns the agent's isolated group and registry membership from spawn
    /// until confirmed settlement, including failed handoff.
    child_group: Option<magistr_process::ProcessGroup>,
    stderr_tail: StderrTail,
    effect_port: Option<Arc<dyn EffectPort>>,
    /// The local port's source route, when the host opted into local effects.
    local_effects: Option<SourceRoute>,
    effects: Option<Arc<EffectBridge>>,
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
            background_tasks: Vec::new(),
            child_group: None,
            stderr_tail: StderrTail::default(),
            effect_port: None,
            local_effects: None,
            effects: None,
        })
    }

    /// Configure the application's capability implementation before connecting.
    /// Bare transports advertise no filesystem or terminal capabilities.
    #[must_use]
    pub fn effect_port(mut self, port: Arc<dyn EffectPort>) -> Self {
        self.effect_port = Some(port);
        self.local_effects = None;
        self
    }

    /// Opt into governed local workspace and terminal effects. Each admission
    /// is resolved through the application's permission receiver; authorized
    /// source writes take `source` — the host's admitted workspace pipeline,
    /// or a refusal that also withholds the write capability.
    #[must_use]
    pub fn local_effects(mut self, source: SourceRoute) -> Self {
        self.effect_port = None;
        self.local_effects = Some(source);
        self
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

    /// Select a model before the session is ready for its first prompt.
    #[must_use]
    pub fn session_model(mut self, model: Option<String>) -> Self {
        self.set_session_choice("model", model);
        self
    }

    /// Prefer a model without requiring it: the nearest offered model (the
    /// same family under a newer release id) is selected, and an id the agent
    /// no longer offers keeps the agent's default with a warning. An explicit
    /// [`Self::session_model`] choice always wins. Set before `connect()`.
    #[must_use]
    pub fn session_model_preferred(mut self, model: Option<String>) -> Self {
        self.set_session_choice("preferred_model", model);
        self
    }

    /// Select reasoning effort, rejecting providers that cannot apply it.
    #[must_use]
    pub fn session_effort(mut self, effort: Option<String>) -> Self {
        self.set_session_choice("effort", effort);
        self
    }

    /// Supply provider-specific system instructions for this session.
    #[must_use]
    pub fn session_system_prompt(mut self, prompt: Option<String>) -> Self {
        self.set_session_choice("system_prompt", prompt);
        self
    }

    fn set_session_choice(&mut self, key: &str, value: Option<String>) {
        let meta = self.session_meta.get_or_insert_with(Meta::new);
        let magistr = meta
            .entry("magistr".to_owned())
            .or_insert_with(|| serde_json::json!({}));
        // This namespace is owned by Magistr; preserve unrelated metadata.
        if !magistr.is_object() {
            *magistr = serde_json::json!({});
        }
        let session = magistr
            .as_object_mut()
            .expect("object ensured")
            .entry("session")
            .or_insert_with(|| serde_json::json!({}));
        if !session.is_object() {
            *session = serde_json::json!({});
        }
        let choices = session.as_object_mut().expect("object ensured");
        if let Some(value) = value {
            choices.insert(key.to_owned(), value.into());
        } else {
            choices.remove(key);
        }
    }

    /// Spawn the agent and complete the handshake.
    pub async fn connect(&mut self) -> Result<()> {
        self.connect_checkpointed(|| Ok(())).await
    }

    /// Persist the caller's launch marker immediately before spawning.
    /// A failed checkpoint must never create a provider process.
    pub async fn connect_checkpointed(
        &mut self,
        before_spawn: impl FnOnce() -> Result<()>,
    ) -> Result<()> {
        if self.is_ready() {
            return Ok(());
        }
        self.close_previous_connection().await?;
        let command = spawn::build_command(&self.cli_path, &self.cli_args, self.cwd.as_deref());
        before_spawn()?;
        let spawned = spawn::spawn_agent(command)?;
        self.child_group = spawned.group;
        self.background_tasks
            .push(drain_stderr(spawned.stderr, self.stderr_tail.clone()));
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
        self.close_previous_connection().await?;
        self.start(component, None).await
    }

    async fn close_previous_connection(&mut self) -> Result<()> {
        if self.connection.is_some() || !self.background_tasks.is_empty() {
            self.close().await?;
        }
        Ok(())
    }

    async fn start(
        &mut self,
        component: impl ConnectTo<Client> + 'static,
        child: Option<magistr_process::ReapingChild>,
    ) -> Result<()> {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (perm_tx, perm_rx) = mpsc::unbounded_channel();
        // The handshake outcome travels on its own channel so connect() never
        // drains event_rx — consumers expect the Initialized event there.
        let (handshake_tx, handshake_rx) = oneshot::channel();

        let cwd = match &self.cwd {
            Some(cwd) => cwd.clone(),
            None => std::env::current_dir().map_err(|error| {
                AcpError::connection(format!("cannot determine ACP working directory: {error}"))
            })?,
        };
        let cwd = tokio::fs::canonicalize(&cwd).await.map_err(|error| {
            AcpError::connection(format!(
                "cannot resolve ACP working directory {}: {error}",
                cwd.display()
            ))
        })?;
        let port = match &self.local_effects {
            Some(source) => Some(Arc::new(LocalEffectPort::new(
                cwd.clone(),
                event_tx.clone(),
                source.clone(),
            )) as Arc<dyn EffectPort>),
            None => self.effect_port.clone(),
        };
        let effects = Arc::new(EffectBridge::new(port, perm_tx, event_tx.clone()));
        self.effects = Some(Arc::clone(&effects));
        let inputs = ConnectionInputs {
            child,
            cwd,
            session_mode: self.session_mode.clone(),
            mcp_servers: self.mcp_servers.clone(),
            session_meta: self.session_meta.clone(),
            cmd_rx,
            event_tx,
            ready: Arc::clone(&self.ready),
            handshake_tx,
            stderr_tail: self.stderr_tail.clone(),
            effects,
        };
        // Own the task before awaiting: cancelling connect must leave Drop able
        // to abort a handshake that never completed.
        self.cmd_tx = Some(cmd_tx);
        self.event_rx = Some(event_rx);
        self.perm_rx = Some(perm_rx);
        self.connection = Some(tokio::spawn(connection::run_connection(component, inputs)));

        let error = match tokio::time::timeout(HANDSHAKE_TIMEOUT, handshake_rx).await {
            Ok(Ok(Ok(()))) => None,
            Ok(Ok(Err(e))) => Some(e),
            Ok(Err(_task_gone)) => Some(AcpError::connection(self.stderr_tail.append_to(
                "ACP connection task exited before the agent handshake completed".to_string(),
            ))),
            Err(_elapsed) => Some(AcpError::timeout(self.stderr_tail.append_to(format!(
                "ACP agent did not complete its handshake within {} seconds",
                HANDSHAKE_TIMEOUT.as_secs()
            )))),
        };
        if let Some(error) = error {
            if let Some(connection) = &self.connection {
                connection.abort();
            }
            // The handshake error is the useful cause; close still joins the
            // aborted task and cleans every owned process and background task.
            let _ = self.close().await;
            return Err(error);
        }

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
        Ok(AcpSender::new(
            Arc::clone(&self.ready),
            cmd_tx.clone(),
            self.effects.clone(),
        ))
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
        self.effects
            .as_ref()
            .ok_or_else(|| {
                AcpError::transport(format!(
                    "cannot kill terminal {id}: effect capability is not connected"
                ))
            })?
            .kill_terminal(id)
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

    /// Send a vendor extension whose completion arrives on the event stream;
    /// see [`AcpSender::extension_observed`].
    ///
    /// # Errors
    ///
    /// When the session is not live. Request failures arrive as a correlated
    /// [`AcpEvent::ExtensionResponse`] instead.
    pub fn extension_observed(
        &self,
        request_id: u64,
        method: &str,
        params: serde_json::Value,
    ) -> Result<()> {
        self.sender()?
            .extension_observed(request_id, method, params)
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
        self.background_tasks.push(tokio::spawn(async move {
            while let Some(item) = source.recv().await {
                if let Ok(event) = &item {
                    let _ = tap_tx.send(event.clone());
                }
                if forward_tx.send(item).is_err() {
                    break;
                }
            }
        }));
        self.event_rx = Some(forward_rx);
        Some(tap_rx)
    }

    /// The permission-request receiver — `Some` on the first call after
    /// `connect()`. Whoever takes it must answer (or drop) every request.
    pub fn take_permission_rx(&mut self) -> Option<mpsc::UnboundedReceiver<AcpPermissionRequest>> {
        self.perm_rx.take()
    }

    /// Owned process group; `0` before spawn, after confirmed settlement,
    /// or for an in-process component. Failed cleanup retains the group.
    pub fn child_pgid(&self) -> i32 {
        self.child_group
            .as_ref()
            .map_or(0, magistr_process::ProcessGroup::id)
    }

    /// `SIGKILL` the agent's whole process group. Idempotent; no-op off Unix
    /// or before spawn.
    fn kill_child_process_group(&self) -> Result<()> {
        self.child_group
            .as_ref()
            .map_or(Ok(()), |group| group.kill())
            .map_err(|error| {
                AcpError::transport(format!("Failed to kill ACP agent process group: {error}"))
            })
    }

    /// Cancel any turn in flight, let the connection wind down, and make sure
    /// the agent is gone. A timeout still aborts and joins the connection;
    /// timeout and task failure are returned after cleanup.
    pub async fn close(&mut self) -> Result<()> {
        self.ready.store(false, Ordering::SeqCst);
        if let Some(effects) = self.effects.take() {
            effects.close();
        }
        if let Some(tx) = self.cmd_tx.take() {
            let _ = tx.send(AcpCommand::Close);
        }
        let outcome = self.finish_connection().await;
        // Settlement is authoritative: a second signal can race reaping and
        // report EPERM for an already-exiting group on Darwin.
        let _ = self.kill_child_process_group();
        let background_outcome = self.stop_background_tasks().await;
        let settlement = match self.child_group.as_mut() {
            Some(group) => group
                .wait_for_settlement(Duration::from_secs(2))
                .await
                .map_err(|error| {
                    AcpError::transport(format!("ACP process cleanup incomplete: {error}"))
                }),
            None => Ok(()),
        };
        if settlement.is_ok() {
            self.child_group = None;
        }
        settlement.and(background_outcome).and(outcome)
    }

    async fn finish_connection(&mut self) -> Result<()> {
        let Some(connection) = self.connection.as_mut() else {
            return Ok(());
        };
        // Borrow the handle so a timeout or cancellation cannot detach it.
        let joined = tokio::time::timeout(CLOSE_TIMEOUT, connection).await;
        let outcome = match joined {
            Ok(joined) => joined.map_err(|error| {
                AcpError::transport(format!("ACP connection task failed: {error}"))
            }),
            Err(_elapsed) => {
                let _ = self.kill_child_process_group();
                let connection = self.connection.as_mut().expect("connection still owned");
                connection.abort();
                let joined = connection.await;
                if let Err(error) = joined
                    && !error.is_cancelled()
                {
                    Err(AcpError::transport(format!(
                        "ACP connection task failed during shutdown: {error}"
                    )))
                } else {
                    Err(AcpError::timeout(format!(
                        "ACP connection did not close within {} seconds; aborted and joined",
                        CLOSE_TIMEOUT.as_secs()
                    )))
                }
            }
        };
        self.connection = None;
        outcome
    }

    async fn stop_background_tasks(&mut self) -> Result<()> {
        let mut outcome = Ok(());
        for task in &self.background_tasks {
            task.abort();
        }
        // Retain each handle across await so cancelled close still leaves Drop
        // with all unfinished tasks. These tasks have no returned result.
        while let Some(task) = self.background_tasks.last_mut() {
            if let Err(error) = task.await
                && !error.is_cancelled()
            {
                tracing::warn!(%error, "ACP background task failed during shutdown");
                if outcome.is_ok() {
                    outcome = Err(AcpError::transport(format!(
                        "ACP background task failed during shutdown: {error}"
                    )));
                }
            }
            self.background_tasks.pop();
        }
        outcome
    }
}

impl Drop for AcpTransport {
    fn drop(&mut self) {
        self.ready.store(false, Ordering::SeqCst);
        if let Some(effects) = self.effects.take() {
            effects.close();
        }
        // Kill first: works even when the task is wedged or we are unwinding.
        let _ = self.kill_child_process_group();
        if let Some(tx) = self.cmd_tx.take() {
            let _ = tx.send(AcpCommand::Close);
        }
        if let Some(connection) = self.connection.take() {
            connection.abort();
        }
        for task in &self.background_tasks {
            task.abort();
        }
    }
}

/// Keep the agent's last stderr lines (and drain the pipe so it never blocks).
fn drain_stderr(stderr: tokio::process::ChildStderr, tail: StderrTail) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut lines = tokio::io::BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            tracing::debug!(target: "acp_agent_stderr", "{line}");
            tail.push(line);
        }
    })
}

#[cfg(test)]
#[path = "mod.tests.rs"]
mod tests;
