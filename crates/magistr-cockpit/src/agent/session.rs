//! The live session: the transport's events onto the inbox, the core's
//! commands onto the transport.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use agent_client_protocol::schema::v1::{
    SessionConfigOption, SessionConfigOptionCategory, SessionConfigOptionValue, ToolKind,
};
use magistr_acp::{
    AcpEvent, AcpPermissionOption, AcpPermissionOptionKind, AcpPermissionRequest,
    AcpPermissionResponse, AcpTransport,
};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::task::JoinHandle;

use super::event::AgentEvent;
use super::permission_choice::PermissionChoice;
use super::permission_kind::PermissionKind;
use super::policy_decision::PolicyDecision;
use super::session_options::SessionOptions;
use super::session_policy::SessionPolicy;
use super::tool_status::ToolStatus;
use super::translate::translate;
use crate::channel::{Commands, Inbox};
use crate::checkpoint::CheckpointPort;
use crate::cockpit::CheckpointCmd;
use crate::cockpit::{Cmd, KillCmd, Msg};
use crate::engine::EnginePort;
use crate::workbench::WorkbenchPort;

/// Run a connected `transport` as the cockpit's session until the loop
/// is gone.
///
/// Every event the transport yields is translated and posted on `inbox`
/// as [`Msg::Agent`]; on `Initialized` the model in `options` is selected
/// (when the agent offers one) and the opening prompt sent, echoed on the
/// stage as the user's text. Every permission request is put to `policy`
/// first — an allow or a deny is answered right here — and only an ask
/// becomes [`AgentEvent::PermissionRequest`] with a session-unique id
/// while its reply channel waits here for the matching
/// [`Cmd::Permission`]; a request for a tool the user answered *Always*
/// is allowed without reaching the cockpit. [`Cmd::Prompt`] goes out as
/// a turn, [`Cmd::Cancel`] as `session/cancel`; [`Cmd::Workbench`] is
/// applied on `workbench`, [`Cmd::Engine`] on `engine` and
/// [`Cmd::Checkpoint`] on `checkpoints` when the port was given — the
/// Changes tab's journal, undo and palette answers, the Trace tab's pin
/// and checkpoint, and the ledger's save, restore and fork never touch
/// the agent — and dropped otherwise. When `commands` ends — the runtime is gone — the
/// transport is dropped, which ends the agent.
// Eight because this is the session's whole fan-in — the transport, the
// two channels, the options, the policy and the three optional ports —
// and each is a distinct collaborator the caller must decide on. A
// config struct would re-spell the same eight at the one call site per
// entry point and hide which of them are optional.
#[allow(clippy::too_many_arguments)]
pub fn spawn_session(
    mut transport: AcpTransport,
    inbox: Inbox,
    commands: Commands,
    options: SessionOptions,
    policy: Box<dyn SessionPolicy>,
    workbench: Option<Box<dyn WorkbenchPort>>,
    engine: Option<Box<dyn EnginePort>>,
    checkpoints: Option<Box<dyn CheckpointPort>>,
) -> JoinHandle<()> {
    let events = transport.read_events();
    let requests = transport.take_permission_rx();
    let session = Session {
        transport,
        inbox,
        options,
        policy,
        workbench,
        engine,
        checkpoints: checkpoints.map(|port| Arc::new(Mutex::new(port))),
        pending: HashMap::new(),
        next_id: 1,
        allowed: HashSet::new(),
        tool_started: HashMap::new(),
        started: Instant::now(),
    };
    tokio::spawn(session.run(events, requests, commands))
}

struct Session {
    transport: AcpTransport,
    inbox: Inbox,
    options: SessionOptions,
    /// The decision asked for before a request reaches the cockpit.
    policy: Box<dyn SessionPolicy>,
    /// The workbench the cockpit acts through without the agent —
    /// [`McpWorkbench`](crate::workbench::McpWorkbench) from the
    /// project's `.mcp.json` at the entry points, the fake in the
    /// harness; absent when the project configures none.
    workbench: Option<Box<dyn WorkbenchPort>>,
    /// The think engine the Trace tab's pin and checkpoint act through
    /// without the agent — the entry point's in-process engines, or the
    /// harness's fake.
    engine: Option<Box<dyn EnginePort>>,
    /// The session's checkpoint spine — where a state is actually
    /// captured, resumed and forked. The entry point's, or the
    /// harness's fake. Shared rather than owned because its verbs run
    /// on a blocking thread ([`Session::checkpoint`]).
    checkpoints: Option<Arc<Mutex<Box<dyn CheckpointPort>>>>,
    /// Requests shown on the cockpit, by the id it will answer with.
    pending: HashMap<u64, AcpPermissionRequest>,
    next_id: u64,
    /// Tool titles the user allowed for the rest of the session.
    allowed: HashSet<String>,
    /// When each running tool call was announced, by id — the wall
    /// clock the durations on the turn's cards are measured against.
    tool_started: HashMap<String, Instant>,
    /// When this session was spawned — the clock the agent block's
    /// minutes are stamped from on every usage update, so the cockpit
    /// core reads time as data the way it reads a tool's duration.
    started: Instant,
}

impl Session {
    async fn run(
        mut self,
        mut events: Option<UnboundedReceiver<magistr_acp::Result<AcpEvent>>>,
        mut requests: Option<UnboundedReceiver<AcpPermissionRequest>>,
        mut commands: Commands,
    ) {
        loop {
            tokio::select! {
                event = recv(&mut events) => match event {
                    Some(event) => {
                        if !self.event(event).await {
                            return;
                        }
                    }
                    None => events = None,
                },
                request = recv(&mut requests) => match request {
                    Some(request) => {
                        if !self.request(request) {
                            return;
                        }
                    }
                    None => requests = None,
                },
                cmd = commands.next() => match cmd {
                    Some(cmd) => self.command(cmd),
                    None => return,
                },
            }
        }
    }

    /// Post one event; on `Initialized`, select the model and open.
    /// `false` once the loop is gone.
    async fn event(&mut self, event: magistr_acp::Result<AcpEvent>) -> bool {
        let (opening, selected) = match &event {
            Ok(AcpEvent::Initialized {
                session_id,
                config_options,
                ..
            }) => (
                true,
                self.select_model(session_id, config_options.as_deref())
                    .await,
            ),
            _ => (false, None),
        };
        // A request nobody answered was denied by the transport at its
        // timeout (`permission_lapsed`): the card must come down by the
        // id the cockpit knows it by, which only this map holds.
        if let Ok(AcpEvent::System { subtype, data }) = &event
            && subtype == "permission_lapsed"
        {
            return self.lapse(data);
        }
        let mut translated = translate(&event);
        self.stamp(&mut translated);
        // The model the agent accepted outranks the one its option
        // list opened on: the handshake names what the session runs.
        if let (AgentEvent::Initialized { model, .. }, Some(chosen)) = (&mut translated, selected) {
            *model = Some(chosen);
        }
        if !self.inbox.send(Msg::Agent(translated)) {
            return false;
        }
        if opening && let Some(prompt) = self.options.opening_prompt.take() {
            if !self
                .inbox
                .send(Msg::Agent(AgentEvent::UserText(format!("{prompt}\n"))))
            {
                return false;
            }
            self.prompt(&prompt);
        }
        true
    }

    /// Track each tool call's start and stamp the elapsed wall time
    /// onto the update that finishes it — the cockpit core never reads
    /// a clock, so the duration a card shows is measured here, at the
    /// transport.
    fn stamp(&mut self, event: &mut AgentEvent) {
        match event {
            AgentEvent::ToolCall { id, .. } => {
                self.tool_started.insert(id.clone(), Instant::now());
            }
            AgentEvent::ToolCallUpdate {
                id,
                status: Some(ToolStatus::Completed | ToolStatus::Failed),
                duration_ms,
                ..
            } => {
                if let Some(started) = self.tool_started.remove(id) {
                    *duration_ms =
                        Some(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX));
                }
            }
            // The session's own clock rides the metering message, so
            // the agent block's minutes come from the same place the
            // card durations do.
            AgentEvent::Usage { elapsed_secs, .. } => {
                *elapsed_secs = Some(self.started.elapsed().as_secs());
            }
            _ => {}
        }
    }

    /// `session/set_config_option` for the model in the options, when
    /// the agent advertises one; a failure leaves the agent's default.
    /// Answers the model the agent accepted, so the handshake can name
    /// what the session actually runs on.
    async fn select_model(
        &self,
        session_id: &str,
        config_options: Option<&[SessionConfigOption]>,
    ) -> Option<String> {
        let model = self.options.model.as_deref()?;
        let option = config_options.and_then(|options| {
            options
                .iter()
                .find(|option| option.category == Some(SessionConfigOptionCategory::Model))
        })?;
        let value = SessionConfigOptionValue::ValueId {
            value: model.to_owned().into(),
        };
        self.transport
            .set_config_option(session_id.to_owned(), option.id.to_string(), value)
            .await
            .ok()
            .map(|_| model.to_owned())
    }

    /// Send `text` as a turn; a transport failure lands on the stage.
    fn prompt(&self, text: &str) {
        if let Err(e) = self.transport.prompt(text) {
            self.inbox
                .send(Msg::Agent(AgentEvent::Error(e.to_string())));
        }
    }

    /// Forward a permission request to the cockpit, or answer it here —
    /// the policy first (a protected-path deny outranks even the
    /// session's *Always* set), then the *Always* set. `false` once the
    /// loop is gone.
    fn request(&mut self, request: AcpPermissionRequest) -> bool {
        // Decided once: `decide` takes `&mut self` because a policy
        // keeps records (the audit log above all), so asking twice
        // would write the entry twice.
        let risk = match self.policy.decide(&request) {
            PolicyDecision::Allow => {
                let _ = request
                    .response_tx
                    .send(select(&request.options, AcpPermissionOptionKind::AllowOnce));
                return true;
            }
            PolicyDecision::Deny => {
                let _ = request.response_tx.send(select(
                    &request.options,
                    AcpPermissionOptionKind::RejectOnce,
                ));
                return true;
            }
            PolicyDecision::Ask(risk) => risk,
        };
        if self.allowed.contains(&request.tool_title) {
            let _ = request
                .response_tx
                .send(select(&request.options, AcpPermissionOptionKind::AllowOnce));
            return true;
        }
        let id = self.next_id;
        self.next_id += 1;
        let event = AgentEvent::PermissionRequest {
            id,
            title: request.tool_title.clone(),
            choices: request.options.iter().map(PermissionChoice::from).collect(),
            edit: request.tool_kind == Some(ToolKind::Edit),
            risk,
            // ACP permission requests carry a tool title, raw input, and options.
            // They do not supply the structured plan needed to build a plan card.
            plan: None,
        };
        self.pending.insert(id, request);
        self.inbox.send(Msg::Agent(event))
    }

    /// Remove a permission request denied by the transport after it expired.
    ///
    /// Find the card by tool-call ID, discard its reply channel, and post the
    /// lapse with the cockpit's card ID. Requests already answered by policy or
    /// by the user have no pending card. Returns false once the loop is gone.
    fn lapse(&mut self, data: &serde_json::Value) -> bool {
        let call = data.get("tool_call_id").and_then(serde_json::Value::as_str);
        let Some(id) = self
            .pending
            .iter()
            .find_map(|(id, request)| (Some(request.tool_call_id.as_str()) == call).then_some(*id))
        else {
            return true;
        };
        let Some(request) = self.pending.remove(&id) else {
            return true;
        };
        let after_secs = data
            .get("timeout_secs")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        self.inbox.send(Msg::Agent(AgentEvent::PermissionLapsed {
            id,
            title: request.tool_title,
            after_secs,
        }))
    }

    fn command(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Prompt(text) => self.prompt(&text),
            Cmd::Cancel => {
                if let Err(e) = self.transport.cancel() {
                    self.inbox
                        .send(Msg::Agent(AgentEvent::Error(e.to_string())));
                }
            }
            Cmd::Permission { id, option_id } => {
                let Some(request) = self.pending.remove(&id) else {
                    return;
                };
                let kind = request
                    .options
                    .iter()
                    .find(|option| option.option_id == option_id)
                    .map(|option| PermissionKind::from(option.kind));
                if kind == Some(PermissionKind::AllowAlways) {
                    self.allowed.insert(request.tool_title.clone());
                }
                // The charter's half: what the reader
                // answered is the policy's to tally under the ask's
                // scope, and to write as a rule when it stands.
                if let Some(kind) = kind {
                    self.policy.answered(&request, kind);
                }
                let _ = request
                    .response_tx
                    .send(AcpPermissionResponse::Selected(option_id));
            }
            // The reader's words on an ask still pending — sent ahead of
            // the answer, so the request they are about is still here.
            Cmd::Constraint { id, condition } => {
                if let Some(request) = self.pending.get(&id) {
                    self.policy.constrain(request, &condition);
                }
            }
            Cmd::Delegate(key) => {
                self.policy.delegate(&key);
            }
            // The held kill: the agent's terminal is
            // the transport's child; a gate's is the loop driver's,
            // which fronts these commands and never forwards one.
            Cmd::Kill(KillCmd::Terminal(id)) => {
                if let Err(e) = self.transport.kill_terminal(&id) {
                    self.inbox
                        .send(Msg::Agent(AgentEvent::Error(e.to_string())));
                }
            }
            Cmd::Kill(KillCmd::Gate(_)) => {}
            Cmd::Workbench(cmd) => {
                if let Some(port) = self.workbench.as_deref_mut() {
                    let event = crate::workbench::apply(port, &cmd);
                    self.inbox.send(Msg::Workbench(event));
                }
            }
            Cmd::Engine(cmd) => {
                if let Some(port) = self.engine.as_deref_mut() {
                    let event = crate::engine::apply(port, &cmd);
                    self.inbox.send(Msg::Engine(event));
                }
            }
            Cmd::Checkpoint(cmd) => self.checkpoint(cmd),
            // The loop's decisions belong to the driver in front of the
            // session. Quit, announcements and the animation tick the
            // runtime handles itself before commands reach here; a
            // shelf request belongs to the entry point that owns the
            // project directory, which fronts the commands and serves
            // it before they reach here — as does a project being
            // born, which the same front serves.
            Cmd::Quit
            | Cmd::Loop(_)
            | Cmd::Shelf(_)
            | Cmd::Birth(_)
            | Cmd::Announce(_)
            | Cmd::Clipboard(_)
            | Cmd::Reconnect
            | Cmd::Animate(_)
            | Cmd::Still => {}
        }
    }

    /// Run a checkpoint operation off the session relay task.
    ///
    /// Saving reads Git and forking starts a worktree operation, so either may
    /// block. Spawn the work separately and post its result to the inbox. The
    /// checkpoint mutex serializes operations without pausing agent events.
    fn checkpoint(&mut self, cmd: CheckpointCmd) {
        if let Some(port) = &self.checkpoints {
            crate::checkpoint::detach(port, cmd, &self.inbox);
        }
    }
}

/// The option of `kind`, else the first, else a cancel — the transport's
/// own fallback order.
fn select(options: &[AcpPermissionOption], kind: AcpPermissionOptionKind) -> AcpPermissionResponse {
    options
        .iter()
        .find(|option| option.kind == kind)
        .or_else(|| options.first())
        .map_or(AcpPermissionResponse::Cancelled, |option| {
            AcpPermissionResponse::Selected(option.option_id.clone())
        })
}

/// The next item of an optional receiver; a closed one is taken away by
/// the caller and a missing one never resolves, so `select!` never spins.
async fn recv<T>(rx: &mut Option<UnboundedReceiver<T>>) -> Option<T> {
    match rx {
        Some(rx) => rx.recv().await,
        None => std::future::pending().await,
    }
}

#[cfg(test)]
#[path = "session.tests.rs"]
mod tests;
