//! Conversation and planning coordination over neutral session and project ports.
//!
//! The purpose, connector and owner change together: hosts supply admitted provider
//! setup, while this module owns queueing, context isolation and relay lifetime.

use std::{future::Future, pin::Pin, sync::Arc, time::Duration};

use crate::channel::{Commands, Inbox};
use crate::spawn_session;
use magistr_application::sessions::{AgentEvent, SessionHandle};
use magistr_cockpit::checkpoint::CheckpointPort;
use magistr_cockpit::cockpit::{Cmd, LaunchAction, LoopCmd, Msg};
use magistr_cockpit::engine::EnginePort;
use magistr_cockpit::workbench::WorkbenchPort;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::task::JoinHandle;

/// Provider context requested from the host. Planning must use its own policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionPurpose {
    Conversation,
    Roadmap,
}

/// How a conversation phase ended, for the owner of its commands.
#[derive(Debug, PartialEq, Eq)]
pub struct SessionExit {
    /// The cockpit is still there: its commands did not close or quit.
    pub open: bool,
    /// Loop decisions the phase received, returned to their owner.
    pub controls: Vec<LoopCmd>,
    /// The work the reader asked the host to start, when that ended the phase.
    pub launch: Option<LaunchAction>,
}

type Connect = dyn Fn(
        SessionPurpose,
        Option<String>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionHandle>, String>> + Send>>
    + Send
    + Sync;

/// The host's answer to a start: `Ok` when it has the work ready to run once
/// this phase ends, or the refusal in words. Asked before the conversation
/// is given up, so a refused start costs the reader nothing.
type Admit = dyn Fn(&LaunchAction) -> Result<(), String> + Send + Sync;

const CLOSE_GRACE: Duration = Duration::from_secs(5);
const START_WAITS: &str = "Wait for the current response before starting work.";

#[cfg(test)]
#[path = "interactive_session.tests.rs"]
mod tests;

struct Connection {
    task: JoinHandle<Result<(), String>>,
    commands: Option<UnboundedSender<Cmd>>,
    events: UnboundedReceiver<Msg>,
    roadmap: bool,
    replaying: bool,
}

impl Drop for Connection {
    fn drop(&mut self) {
        // Cancelling the outer owner must also release its provider relay.
        // Dropping a JoinHandle alone would leave that session detached.
        self.task.abort();
    }
}

/// Only the active connection can publish into the displayed turn. An idle
/// connection keeps its own channel while its provider context is parked.
async fn connection_event(connection: &mut Option<Connection>) -> Option<Msg> {
    match connection {
        Some(connection) => connection.events.recv().await,
        None => std::future::pending().await,
    }
}

/// Retains the command receiver after a work loop ends and opens a separate
/// planning session for roadmap requests. Ordinary follow-ups reuse their
/// current conversation; switching purpose parks that connection until needed.
pub struct InteractiveSession {
    connect: Arc<Connect>,
    inbox: Inbox,
    local_task: JoinHandle<Result<(), String>>,
    local_commands: Option<UnboundedSender<Cmd>>,
    admit: Option<Arc<Admit>>,
    connection: Option<Connection>,
    parked: Option<Connection>,
    conversation_id: Option<String>,
    roadmap_id: Option<String>,
    busy: bool,
    queued: Option<String>,
    closed: bool,
}

impl Drop for InteractiveSession {
    fn drop(&mut self) {
        // A dropped JoinHandle detaches its relay. Cancel it instead so queued
        // local commands cannot execute after this owner disappears. Connection
        // drops do the same for both active and parked provider relays.
        self.local_task.abort();
    }
}

impl InteractiveSession {
    /// Build an owner with the host's provider connector and admitted local ports.
    /// The connector restores the supplied session identity, or opens a new
    /// session when it is absent. A failed restore must not open a new context.
    /// Restored history precedes the handle's `Initialized` event.
    pub fn new<F, Fut>(
        connect: F,
        inbox: Inbox,
        workbench: Option<Box<dyn WorkbenchPort>>,
        engine: Option<Box<dyn EnginePort>>,
        checkpoints: Option<Box<dyn CheckpointPort>>,
    ) -> Self
    where
        F: Fn(SessionPurpose, Option<String>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Box<dyn SessionHandle>, String>> + Send + 'static,
    {
        let (commands, receiver) = Commands::channel();
        let task = spawn_session(
            Box::new(super::local_session::LocalSession),
            inbox.clone(),
            receiver,
            workbench,
            engine,
            checkpoints,
        );
        Self {
            connect: Arc::new(move |purpose, resume| Box::pin(connect(purpose, resume))),
            inbox,
            local_task: task,
            local_commands: Some(commands),
            admit: None,
            connection: None,
            parked: None,
            conversation_id: None,
            roadmap_id: None,
            busy: false,
            queued: None,
            closed: false,
        }
    }

    /// Let this phase end on a start the host admits ([`Cmd::Launch`]).
    /// Without it every start is refused: nothing behind the session runs work.
    #[must_use]
    pub fn admitting_work(
        mut self,
        admit: impl Fn(&LaunchAction) -> Result<(), String> + Send + Sync + 'static,
    ) -> Self {
        self.admit = Some(Arc::new(admit));
        self
    }

    /// Adopt the opening conversation connected before the interface starts.
    pub fn connected(mut self, handle: Box<dyn SessionHandle>, busy: bool) -> Self {
        self.attach(handle, false, false);
        self.busy = busy;
        self
    }

    fn attach(&mut self, handle: Box<dyn SessionHandle>, roadmap: bool, replaying: bool) {
        let (commands, receiver) = Commands::channel();
        let (inbox, events) = Inbox::channel();
        let task = spawn_session(handle, inbox, receiver, None, None, None);
        self.connection = Some(Connection {
            task,
            commands: Some(commands),
            events,
            roadmap,
            replaying,
        });
    }

    async fn submit(
        &mut self,
        text: String,
        roadmap: bool,
        commands: &mut Commands,
        controls: &mut Vec<LoopCmd>,
    ) {
        if self.busy {
            if roadmap && self.queued.is_none() {
                self.queued = Some(text);
                self.inbox.send(Msg::RoadmapRequestQueued);
            } else {
                self.reject(
                    text,
                    "Wait for the current response before sending another message.",
                );
            }
            return;
        }
        if self
            .connection
            .as_ref()
            .is_some_and(|current| current.roadmap != roadmap)
        {
            // Each purpose keeps its own provider context, including sessions
            // that cannot be loaded again after their connection is closed.
            std::mem::swap(&mut self.connection, &mut self.parked);
        }
        if self.connection.is_none()
            && self
                .parked
                .as_ref()
                .is_some_and(|session| session.roadmap == roadmap)
        {
            std::mem::swap(&mut self.connection, &mut self.parked);
        }
        if self.connection.is_none() {
            let purpose = if roadmap {
                SessionPurpose::Roadmap
            } else {
                SessionPurpose::Conversation
            };
            let resume = if roadmap {
                &self.roadmap_id
            } else {
                &self.conversation_id
            }
            .clone();
            let replaying = resume.is_some();
            let connect =
                tokio::time::timeout(Duration::from_secs(30), (self.connect)(purpose, resume));
            tokio::pin!(connect);
            let connected = loop {
                tokio::select! {
                    result = &mut connect => match result {
                        Ok(result) => break result,
                        Err(_) => {
                            self.reject(text, "The agent took too long to connect. Send your message again.");
                            return;
                        }
                    },
                    command = commands.next() => match command {
                        None | Some(Cmd::Quit) => {
                            self.closed = true;
                            return;
                        }
                        Some(Cmd::CancelRoadmapPrompt) if roadmap => {
                            self.inbox.send(Msg::RoadmapRequestCancelled);
                            return;
                        }
                        Some(Cmd::Cancel) if !roadmap => {
                            self.inbox.send(Msg::Agent(AgentEvent::TurnEnded(
                                magistr_application::sessions::TurnEnd::Cancelled,
                            )));
                            return;
                        }
                        Some(Cmd::Loop(command)) => {
                            let abort = matches!(command, LoopCmd::Abort);
                            controls.push(command);
                            if abort {
                                if roadmap {
                                    self.inbox.send(Msg::RoadmapRequestCancelled);
                                } else {
                                    self.inbox.send(Msg::Agent(AgentEvent::TurnEnded(
                                        magistr_application::sessions::TurnEnd::Cancelled,
                                    )));
                                }
                                return;
                            }
                        }
                        Some(Cmd::Prompt(other) | Cmd::RoadmapPrompt(other)) => {
                            self.reject(other, "Wait for the connection before sending another message.");
                        }
                        Some(Cmd::Launch(action)) => self.refuse_start(action, START_WAITS.to_owned()),
                        Some(other) => { self.forward(other); }
                    }
                }
            };
            match connected {
                Ok(handle) => self.attach(handle, roadmap, replaying),
                Err(error) => {
                    self.reject(text, &format!("Magistr could not connect: {error}"));
                    return;
                }
            }
        }
        if roadmap {
            self.inbox.send(Msg::RoadmapRequestStarted);
        }
        self.busy = true;
        if !self.forward(Cmd::Prompt(text)) {
            self.busy = false;
            self.close().await;
        }
    }

    // Retain identities even after their connection has closed. Replay belongs
    // to the provider's restored context, not to a second displayed response.
    fn observe(&mut self, event: &Msg) -> bool {
        let Some(connection) = self.connection.as_mut() else {
            return true;
        };
        if let Msg::Agent(AgentEvent::Initialized { session_id, .. }) = event {
            let saved = if connection.roadmap {
                &mut self.roadmap_id
            } else {
                &mut self.conversation_id
            };
            *saved = Some(session_id.clone());
            connection.replaying = false;
        }
        !connection.replaying
            || matches!(
                event,
                Msg::Agent(AgentEvent::Error(_) | AgentEvent::PromptRejected { .. })
            )
    }

    fn reject(&self, text: String, error: &str) {
        self.inbox.send(Msg::Agent(AgentEvent::PromptRejected {
            text,
            error: error.to_owned(),
        }));
    }

    fn forward(&self, cmd: Cmd) -> bool {
        let local = matches!(cmd, Cmd::Engine(_) | Cmd::Workbench(_) | Cmd::Checkpoint(_));
        let sender = if local {
            self.local_commands.as_ref()
        } else {
            self.connection
                .as_ref()
                .and_then(|current| current.commands.as_ref())
        };
        let refused = match sender {
            Some(sender) => match sender.send(cmd) {
                Ok(()) => return true,
                Err(error) => error.0,
            },
            None => cmd,
        };
        self.inbox.send(Msg::CommandFailed {
            cmd: refused,
            error: if local {
                "The project connection closed. Reopen the session and try again."
            } else {
                "The agent connection closed. Send a message to reconnect, then try again."
            }
            .into(),
        });
        false
    }

    async fn close(&mut self) {
        if let Some(mut connection) = self.connection.take() {
            drop(connection.commands.take());
            if tokio::time::timeout(CLOSE_GRACE, &mut connection.task)
                .await
                .is_err()
            {
                connection.task.abort();
                let _ = (&mut connection.task).await;
            }
        }
        self.busy = false;
    }

    /// Run until the cockpit closes, or one requested turn ends at an iteration
    /// boundary. Loop controls are returned to their owner rather than discarded.
    ///
    /// A start ([`Cmd::Launch`]) the host admits ends an idle conversation
    /// phase with the commands still open: the host runs the work and opens
    /// the next phase. A follow-up owned by running work (`once`) refuses it,
    /// as do a response in flight and a session nobody admits work for.
    pub async fn run(
        mut self,
        commands: &mut Commands,
        first: Option<Cmd>,
        once: bool,
    ) -> SessionExit {
        let mut first = first;
        let mut controls = Vec::new();
        let mut closed = false;
        let mut launch = None;
        loop {
            let command = if let Some(command) = first.take() {
                Some(command)
            } else {
                tokio::select! {
                    biased;
                    event = connection_event(&mut self.connection) => {
                        let Some(event) = event else {
                            self.busy = false;
                            self.inbox.send(Msg::Agent(AgentEvent::Error(
                                "The agent connection closed. Send your message again to reconnect.".into(),
                            )));
                            self.close().await;
                            if let Some(text) = self.queued.take() {
                                first = Some(Cmd::RoadmapPrompt(text));
                            } else if once {
                                break;
                            }
                            continue;
                        };
                        if !self.observe(&event) {
                            continue;
                        }
                        self.inbox.send(event.clone());
                        if matches!(event, Msg::Agent(
                            AgentEvent::TurnEnded(_) | AgentEvent::Error(_)
                            | AgentEvent::PromptRejected { .. }
                        )) {
                            self.busy = false;
                            if let Some(text) = self.queued.take() {
                                first = Some(Cmd::RoadmapPrompt(text));
                            } else if once {
                                break;
                            }
                        }
                        if matches!(event, Msg::Agent(AgentEvent::Error(_))) {
                            self.close().await;
                        }
                        continue;
                    }
                    command = commands.next() => command,
                }
            };
            let Some(command) = command else {
                closed = true;
                break;
            };
            match command {
                Cmd::Prompt(text) => self.submit(text, false, commands, &mut controls).await,
                Cmd::RoadmapPrompt(text) => self.submit(text, true, commands, &mut controls).await,
                Cmd::CancelRoadmapPrompt => {
                    if self.queued.take().is_some() {
                        self.inbox.send(Msg::RoadmapRequestCancelled);
                    } else if self
                        .connection
                        .as_ref()
                        .is_some_and(|session| session.roadmap)
                        && self.busy
                    {
                        self.forward(Cmd::Cancel);
                    } else {
                        self.inbox.send(Msg::CommandFailed {
                            cmd: Cmd::CancelRoadmapPrompt,
                            error: "There is no roadmap request to stop.".into(),
                        });
                    }
                }
                Cmd::Loop(command) => {
                    if matches!(command, LoopCmd::Abort) {
                        self.forward(Cmd::Cancel);
                    }
                    controls.push(command);
                }
                Cmd::Launch(action) => {
                    let refusal = if once {
                        Some("Work is already running.".to_owned())
                    } else if self.busy || self.queued.is_some() {
                        Some(START_WAITS.to_owned())
                    } else {
                        match &self.admit {
                            Some(admit) => admit(&action).err(),
                            None => Some("This session cannot start work.".to_owned()),
                        }
                    };
                    match refusal {
                        Some(error) => self.refuse_start(action, error),
                        None => {
                            launch = Some(action);
                            break;
                        }
                    }
                }
                Cmd::Quit => {
                    closed = true;
                    break;
                }
                other => {
                    self.forward(other);
                }
            }
            if !once {
                for command in controls.drain(..) {
                    self.inbox.send(Msg::CommandFailed {
                        cmd: Cmd::Loop(command),
                        error: "There is no running work loop for this action.".into(),
                    });
                }
            }
            if self.closed {
                closed = true;
                break;
            }
            if once && !self.busy && self.queued.is_none() {
                break;
            }
        }
        self.close().await;
        self.connection = self.parked.take();
        self.close().await;
        drop(self.local_commands.take());
        if tokio::time::timeout(CLOSE_GRACE, &mut self.local_task)
            .await
            .is_err()
        {
            self.local_task.abort();
            let _ = (&mut self.local_task).await;
        }
        SessionExit {
            open: !closed,
            controls,
            launch,
        }
    }

    fn refuse_start(&self, action: LaunchAction, error: String) {
        self.inbox.send(Msg::CommandFailed {
            cmd: Cmd::Launch(action),
            error,
        });
    }

    pub fn spawn(self, mut commands: Commands) -> JoinHandle<Result<(), String>> {
        tokio::spawn(async move {
            self.run(&mut commands, None, false).await;
            Ok(())
        })
    }
}
