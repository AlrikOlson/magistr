//! Resident host owner for a resolved ACP session.
//!
//! The supervisor serializes turn admission, permission replies and cancellation.
//! Its task owns the transport across turns; detaching observation never changes
//! provider execution. Both legacy headless facades delegate lifecycle here.

mod command;
mod control;
mod goal;
mod opening;
mod owner;
mod permissions;
mod residency;
mod snapshot;
mod turn_outcome;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use agent_client_protocol::schema::v1::{
    AgentCapabilities, SessionConfigOption, SessionConfigOptionValue, SessionInfo, StopReason,
};
use magistr_acp::AcpTransport;
use magistr_cockpit::agent::{AgentEvent, SessionOptions, SessionPolicy};
use magistr_core::AcpSessionState;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use super::ResolvedSessionSpec;
use super::message::AgentMessage;
use crate::error::{MagistrError, Result};
use command::Command;
pub use control::SessionControl;
use permissions::Permissions;
pub use residency::SessionResidency;
pub use snapshot::SessionSupervisorSnapshot;
pub use turn_outcome::TurnOutcome;

type Messages = mpsc::UnboundedReceiver<Result<AgentMessage>>;
type Policy = (
    Box<dyn magistr_cockpit::agent::SessionPolicy>,
    crate::permissions::NoninteractivePolicy,
);

struct Inner {
    snapshot: SessionSupervisorSnapshot,
    queue: VecDeque<(u64, String)>,
    next_turn: u64,
    completed_turns: u32,
    closing: bool,
    legacy_cancel: bool,
    control_busy: bool,
    goal_turn: Option<u64>,
    capabilities: Option<AgentCapabilities>,
    messages: Option<mpsc::UnboundedSender<Result<AgentMessage>>>,
    cockpit_events: Option<mpsc::UnboundedSender<AgentEvent>>,
}

impl Inner {
    fn emit(&mut self, message: Result<AgentMessage>) {
        if let Err(error) = &message {
            self.emit_cockpit(AgentEvent::Error(error.to_string()));
        }
        if self
            .messages
            .as_ref()
            .is_some_and(|tx| tx.send(message).is_err())
        {
            self.messages = None;
            self.snapshot.client_attached = false;
        }
    }

    fn emit_cockpit(&mut self, event: AgentEvent) {
        if self
            .cockpit_events
            .as_ref()
            .is_some_and(|tx| tx.send(event).is_err())
        {
            self.cockpit_events = None;
            self.snapshot.client_attached = false;
        }
    }

    fn discard_queue(&mut self, reason: &str) {
        let turn_ids: Vec<_> = self.queue.iter().map(|(id, _)| *id).collect();
        let count = turn_ids.len();
        self.queue.clear();
        self.snapshot.queued_turns = 0;
        if count > 0 {
            self.emit(Ok(AgentMessage::System {
                subtype: "turns_discarded".into(),
                data: serde_json::json!({"count": count, "turn_ids": turn_ids, "reason": reason}),
            }));
        }
    }

    fn finish(&mut self, residency: SessionResidency, error: &str) {
        self.discard_queue(error);
        if let Some(turn_id) = self.snapshot.active_turn.take() {
            self.snapshot.last_outcome = Some(TurnOutcome {
                turn_id,
                stop_reason: None,
                error: Some(error.to_owned()),
            });
        }
        self.snapshot.cancellation_requested = false;
        self.snapshot.residency = residency;
        self.closing = true;
        self.control_busy = false;
        self.goal_turn = None;
    }

    fn check_admission(&self) -> Result<()> {
        if self.closing || self.snapshot.residency != SessionResidency::Resident {
            return Err(acp_error("Session is no longer accepting input"));
        }
        if self.snapshot.cancellation_requested || self.legacy_cancel {
            return Err(acp_error(
                "Wait for the current cancellation response before submitting a new turn",
            ));
        }
        Ok(())
    }
}

fn acp_error(message: impl Into<String>) -> MagistrError {
    MagistrError::Acp(message.into())
}

/// Owns session residency and commands; dropping it releases the provider.
pub struct SessionSupervisor {
    commands: mpsc::UnboundedSender<Command>,
    inner: Arc<Mutex<Inner>>,
    task: Option<JoinHandle<Result<()>>>,
    /// The provider process the owner registered, when the transport
    /// spawned one — the runtime's own handle, for a dispatch record.
    provider_pid: Option<u32>,
}

impl SessionSupervisor {
    /// Connect the frozen runtime and transfer its policy to the sole session owner.
    pub async fn start(spec: ResolvedSessionSpec) -> Result<(Self, Messages)> {
        let transport = spec.connect_transport().await?;
        let prompt = spec.prompt().to_owned();
        let (policy, handling) = spec.into_permission_policy();
        Self::start_connected(&prompt, transport, Some((Box::new(policy), handling))).await
    }

    /// Adopt an already connected transport without resolving settings again.
    pub(crate) async fn start_connected(
        prompt: &str,
        mut transport: AcpTransport,
        policy: Option<Policy>,
    ) -> Result<(Self, Messages)> {
        let provider_pid = u32::try_from(transport.child_pgid())
            .ok()
            .filter(|pid| *pid > 0);
        let events = transport
            .read_events()
            .ok_or_else(|| acp_error("ACP event channel not available"))?;
        let permissions = Permissions::new(transport.take_permission_rx(), policy);
        let opening = !prompt.trim().is_empty();
        if opening {
            transport
                .prompt(prompt)
                .map_err(|error| acp_error(format!("Failed to send initial prompt: {error}")))?;
        }
        let (messages, receiver) = mpsc::unbounded_channel();
        let (commands, command_rx) = mpsc::unbounded_channel();
        let inner = Arc::new(Mutex::new(Inner {
            snapshot: SessionSupervisorSnapshot {
                active_turn: opening.then_some(1),
                queued_turns: 0,
                cancellation_requested: false,
                residency: SessionResidency::Resident,
                last_outcome: None,
                client_attached: true,
            },
            queue: VecDeque::new(),
            next_turn: if opening { 2 } else { 1 },
            completed_turns: 0,
            closing: false,
            legacy_cancel: false,
            control_busy: false,
            goal_turn: None,
            capabilities: None,
            messages: Some(messages),
            cockpit_events: None,
        }));
        let state = Arc::clone(&inner);
        let task = tokio::spawn(owner::run(
            transport,
            events,
            permissions,
            command_rx,
            state,
            None,
        ));
        Ok((
            Self {
                commands,
                inner,
                task: Some(task),
                provider_pid,
            },
            receiver,
        ))
    }

    /// Adopt a connected transport for an interactive attachment; the same owner
    /// serializes all provider traffic for interactive and headless clients.
    pub(crate) fn start_interactive(
        mut transport: AcpTransport,
        options: SessionOptions,
        policy: Box<dyn SessionPolicy>,
    ) -> Result<(Self, mpsc::UnboundedReceiver<AgentEvent>)> {
        let provider_pid = u32::try_from(transport.child_pgid())
            .ok()
            .filter(|pid| *pid > 0);
        let events = transport
            .read_events()
            .ok_or_else(|| acp_error("ACP event channel not available"))?;
        let (cockpit_events, receiver) = mpsc::unbounded_channel();
        let permissions = Permissions::interactive(
            transport.take_permission_rx(),
            policy,
            cockpit_events.clone(),
        );
        let (commands, command_rx) = mpsc::unbounded_channel();
        let opening = options
            .opening_prompt
            .as_ref()
            .filter(|prompt| !prompt.trim().is_empty());
        let inner = Arc::new(Mutex::new(Inner {
            snapshot: SessionSupervisorSnapshot {
                active_turn: None,
                queued_turns: usize::from(opening.is_some()),
                cancellation_requested: false,
                residency: SessionResidency::Resident,
                last_outcome: None,
                client_attached: true,
            },
            queue: opening
                .map(|prompt| (1, prompt.clone()))
                .into_iter()
                .collect(),
            next_turn: if opening.is_some() { 2 } else { 1 },
            completed_turns: 0,
            closing: false,
            legacy_cancel: false,
            control_busy: true,
            goal_turn: None,
            capabilities: None,
            messages: None,
            cockpit_events: Some(cockpit_events),
        }));
        let task = tokio::spawn(owner::run(
            transport,
            events,
            permissions,
            command_rx,
            Arc::clone(&inner),
            Some(options),
        ));
        Ok((
            Self {
                commands,
                inner,
                task: Some(task),
                provider_pid,
            },
            receiver,
        ))
    }

    /// Narrow host-only goal operations routed through the resident owner.
    #[must_use]
    pub fn control(&self) -> SessionControl {
        SessionControl::new(self.commands.clone(), Arc::clone(&self.inner))
    }

    /// Route an interactive permission answer through the session owner.
    pub fn answer_permission(&self, id: u64, option_id: &str) -> Result<()> {
        self.send_command(Command::Permission {
            id,
            option_id: option_id.to_owned(),
        })
    }

    /// Attach a condition to a live permission request.
    pub fn constrain_permission(&self, id: u64, condition: &str) -> Result<()> {
        self.send_command(Command::Constraint {
            id,
            condition: condition.to_owned(),
        })
    }

    /// Apply the policy's delegation operation in lifecycle order.
    pub fn delegate_permission(&self, key: &str) -> Result<()> {
        self.send_command(Command::Delegate(key.to_owned()))
    }

    /// Kill one terminal through the resident transport owner.
    pub fn kill_terminal(&self, id: &str) -> Result<()> {
        self.send_command(Command::KillTerminal(id.to_owned()))
    }

    /// Admit a follow-up in FIFO order. At most one prompt reaches the provider.
    pub fn send_followup(&self, content: &str) -> Result<()> {
        let mut inner = self.inner.lock().expect("supervisor state lock poisoned");
        inner.check_admission()?;
        let id = inner.next_turn;
        inner.next_turn += 1;
        inner.queue.push_back((id, content.to_owned()));
        inner.snapshot.queued_turns = inner.queue.len();
        if self.commands.send(Command::Wake).is_err() {
            inner.finish(
                SessionResidency::Failed,
                "Session owner stopped before admitting the queued turn",
            );
            return Err(acp_error("Session owner is no longer available"));
        }
        Ok(())
    }

    /// Request cancellation, clear queued input, and retain session residency.
    /// The provider's actual prompt response establishes its terminal outcome.
    pub fn cancel_turn(&self) -> Result<()> {
        self.request_cancel(false)
    }

    fn request_cancel(&self, legacy: bool) -> Result<()> {
        let mut inner = self.inner.lock().expect("supervisor state lock poisoned");
        if inner.closing || inner.snapshot.residency != SessionResidency::Resident {
            return Err(acp_error("Session owner is no longer available"));
        }
        let turn_id = inner.snapshot.active_turn;
        inner.snapshot.cancellation_requested = turn_id.is_some();
        inner.legacy_cancel |= legacy;
        inner.discard_queue("turn cancellation requested");
        self.commands
            .send(Command::Cancel { turn_id })
            .map_err(|_| acp_error("Session owner is no longer available"))
    }

    /// Compatibility cancellation: end observation after a real terminal response.
    pub fn cancel(&mut self) {
        if let Err(error) = self.request_cancel(true) {
            self.inner
                .lock()
                .expect("supervisor state lock poisoned")
                .emit(Err(error));
        }
    }

    /// Stop the owned connection and await all transport cleanup.
    pub async fn terminate_session(&mut self) -> Result<()> {
        {
            let mut inner = self.inner.lock().expect("supervisor state lock poisoned");
            inner.closing = true;
            inner.discard_queue("session termination requested");
        }
        let _ = self.commands.send(Command::Terminate);
        if let Some(task) = self.task.as_mut() {
            let result = task.await;
            self.task.take();
            match result {
                Ok(result) => result?,
                Err(error) if error.is_cancelled() => {}
                Err(error) => {
                    return Err(acp_error(format!("Session owner cleanup failed: {error}")));
                }
            }
        }
        Ok(())
    }

    /// Detach only message observation. The provider and permission owner remain resident.
    pub fn detach_client(&mut self) {
        let _ = self.commands.send(Command::Detach);
        let mut inner = self.inner.lock().expect("supervisor state lock poisoned");
        inner.messages = None;
        inner.cockpit_events = None;
        inner.snapshot.client_attached = false;
    }

    /// Abort residency immediately; preserve any previously confirmed terminal outcome.
    pub fn abort(&mut self) {
        let mut inner = self.inner.lock().expect("supervisor state lock poisoned");
        if inner.snapshot.residency == SessionResidency::Resident {
            inner.finish(
                SessionResidency::Terminated,
                "Session owner dropped before a provider terminal response",
            );
        }
        inner.messages = None;
        inner.cockpit_events = None;
        inner.snapshot.client_attached = false;
        if let Some(task) = self.task.as_ref() {
            task.abort();
        }
    }

    /// The provider process this owner supervises, when the transport
    /// spawned one. A handle for a dispatch record, not proof the runtime
    /// is still the same process.
    #[must_use]
    pub fn provider_pid(&self) -> Option<u32> {
        self.provider_pid
    }

    /// Read consistent admission, outcome and residency facts.
    #[must_use]
    pub fn snapshot(&self) -> SessionSupervisorSnapshot {
        self.inner
            .lock()
            .expect("supervisor state lock poisoned")
            .snapshot
            .clone()
    }

    /// Completed ACP prompt responses, including acknowledged cancellation.
    #[must_use]
    pub fn turn_count(&self) -> u32 {
        self.inner
            .lock()
            .expect("supervisor state lock poisoned")
            .completed_turns
    }

    /// Compatibility projection; use the snapshot to distinguish residency and outcome.
    #[must_use]
    pub fn session_state(&self) -> AcpSessionState {
        let snapshot = self.snapshot();
        if snapshot.active_turn.is_some() {
            AcpSessionState::InProgress
        } else if snapshot.residency == SessionResidency::Failed {
            AcpSessionState::Failed
        } else if let Some(outcome) = snapshot.last_outcome {
            match outcome.stop_reason {
                Some(StopReason::Cancelled) => AcpSessionState::Cancelled,
                Some(StopReason::EndTurn) if snapshot.residency == SessionResidency::Resident => {
                    AcpSessionState::Awaiting
                }
                Some(StopReason::EndTurn) => AcpSessionState::Completed,
                Some(_) | None => AcpSessionState::Failed,
            }
        } else {
            AcpSessionState::Created
        }
    }

    /// Negotiated provider capabilities once initialization has been observed.
    pub fn agent_capabilities(&self) -> Option<AgentCapabilities> {
        self.inner
            .lock()
            .expect("supervisor state lock poisoned")
            .capabilities
            .clone()
    }

    fn send_command(&self, command: Command) -> Result<()> {
        if self.commands.send(command).is_err() {
            let mut inner = self.inner.lock().expect("supervisor state lock poisoned");
            inner.finish(
                SessionResidency::Failed,
                "Session owner is no longer available",
            );
            return Err(acp_error("Session owner is no longer available"));
        }
        Ok(())
    }

    fn reserve_control(&self) -> Result<()> {
        let mut inner = self.inner.lock().expect("supervisor state lock poisoned");
        inner.check_admission()?;
        if inner.snapshot.active_turn.is_some() || !inner.queue.is_empty() || inner.control_busy {
            return Err(acp_error(
                "Cannot change or query session configuration while a prompt or configuration request is in-flight",
            ));
        }
        inner.control_busy = true;
        Ok(())
    }

    /// List sessions between turns; configuration traffic never blocks cancellation cleanup.
    pub async fn list_sessions(&self) -> Result<Vec<SessionInfo>> {
        self.reserve_control()?;
        let (reply, response) = oneshot::channel();
        self.send_command(Command::List { reply })?;
        response
            .await
            .map_err(|_| acp_error("list_sessions response channel dropped"))?
    }

    /// Apply a configuration change only while the session is idle.
    pub async fn set_config_option(
        &self,
        config_id: String,
        value: SessionConfigOptionValue,
    ) -> Result<Vec<SessionConfigOption>> {
        self.reserve_control()?;
        let (reply, response) = oneshot::channel();
        self.send_command(Command::Config {
            config_id,
            value,
            reply: Some(reply),
        })?;
        response
            .await
            .map_err(|_| acp_error("set_config_option response channel dropped"))?
    }

    /// Queue an idle configuration change; asynchronous refusal is emitted on the message stream.
    pub fn send_config_change(&self, config_id: String, value_id: String) -> Result<()> {
        self.reserve_control()?;
        self.send_command(Command::Config {
            config_id,
            value: SessionConfigOptionValue::ValueId {
                value: value_id.into(),
            },
            reply: None,
        })
    }
}

impl Drop for SessionSupervisor {
    fn drop(&mut self) {
        self.abort();
    }
}

#[cfg(test)]
#[path = "../session_supervisor.tests.rs"]
mod tests;
