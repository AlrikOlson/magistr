//! Intent-aware conversation sessions shared by live command entry points.

use std::time::Duration;

use crate::application::workflow_client::EngineClient;
use magistr_cockpit::agent::{AgentEvent, SessionOptions, spawn_session};
use magistr_cockpit::channel::{Commands, Inbox};
use magistr_cockpit::cockpit::{Cmd, LoopCmd, Msg};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::task::JoinHandle;

use super::ledger::{SpinePort, session_engine};
use super::loop_driver::Connector;
use super::roadmap_request::RoadmapPolicy;
use crate::agent::AgentConfig;
use crate::agent::cockpit_session::CockpitSession;

const CLOSE_GRACE: Duration = Duration::from_secs(5);

struct Connection {
    task: JoinHandle<Result<(), String>>,
    commands: Option<UnboundedSender<Cmd>>,
    events: UnboundedReceiver<Msg>,
    roadmap: bool,
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
pub(crate) struct InteractiveSession {
    config: AgentConfig,
    connector: Connector,
    inbox: Inbox,
    local_task: JoinHandle<Result<(), String>>,
    local_commands: UnboundedSender<Cmd>,
    connection: Option<Connection>,
    parked: Option<Connection>,
    busy: bool,
    queued: Option<String>,
    closed: bool,
}

impl InteractiveSession {
    pub(crate) fn new(
        config: AgentConfig,
        connector: Connector,
        inbox: Inbox,
        engines: Option<EngineClient>,
    ) -> Self {
        let checkpoints = engines.as_ref().map(|engines| {
            Box::new(SpinePort::at(
                session_engine(&config.cwd, engines.reader().clone()),
                &config.cwd,
            )) as _
        });
        Self::with_checkpoints(config, connector, inbox, engines, checkpoints)
    }

    pub(crate) fn with_checkpoints(
        config: AgentConfig,
        connector: Connector,
        inbox: Inbox,
        engines: Option<EngineClient>,
        checkpoints: Option<Box<dyn magistr_cockpit::checkpoint::CheckpointPort>>,
    ) -> Self {
        let (commands, receiver) = Commands::channel();
        let root = &config.cwd;
        let task = spawn_session(
            Box::new(super::local_session::LocalSession),
            inbox.clone(),
            receiver,
            super::commands::workbench_port::workbench_port(root),
            engines.as_ref().and_then(EngineClient::port),
            checkpoints,
        );
        Self {
            config,
            connector,
            inbox,
            local_task: task,
            local_commands: commands,
            connection: None,
            parked: None,
            busy: false,
            queued: None,
            closed: false,
        }
    }

    /// Adopt the opening session which was connected before terminal takeover.
    pub(crate) fn connected(mut self, handle: CockpitSession, busy: bool) -> Self {
        self.attach(handle, false);
        self.busy = busy;
        self
    }

    fn attach(&mut self, handle: CockpitSession, roadmap: bool) {
        let (commands, receiver) = Commands::channel();
        let (inbox, events) = Inbox::channel();
        let task = spawn_session(Box::new(handle), inbox, receiver, None, None, None);
        self.connection = Some(Connection {
            task,
            commands: Some(commands),
            events,
            roadmap,
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
            let config = if roadmap {
                super::roadmap_request::config(&self.config)
            } else {
                let mut config = self.config.clone();
                config.prompt.clear();
                config.resume = None;
                config.continue_conversation = false;
                config
            };
            let connector = self.connector.clone();
            let connect = tokio::time::timeout(Duration::from_secs(30), connector.connect(config));
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
                                magistr_cockpit::agent::TurnEnd::Cancelled,
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
                                        magistr_cockpit::agent::TurnEnd::Cancelled,
                                    )));
                                }
                                return;
                            }
                        }
                        Some(Cmd::Prompt(other) | Cmd::RoadmapPrompt(other)) => {
                            self.reject(other, "Wait for the connection before sending another message.");
                        }
                        Some(other) => { self.forward(other); }
                    }
                }
            };
            let handle = match connected {
                Ok((transport, spec)) => {
                    let (policy, _) = spec.into_permission_policy();
                    let policy: Box<dyn magistr_cockpit::agent::SessionPolicy> = if roadmap {
                        Box::new(RoadmapPolicy::new(policy))
                    } else {
                        Box::new(policy)
                    };
                    CockpitSession::start_connected(transport, SessionOptions::new(), policy)
                        .map_err(|error| error.to_string())
                }
                Err(error) => Err(error.to_string()),
            };
            match handle {
                Ok(handle) => self.attach(handle, roadmap),
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
        if !self.forward(Cmd::Prompt(text.clone())) {
            self.busy = false;
            self.reject(
                text,
                "The agent connection closed. Send your message again to reconnect.",
            );
            self.close().await;
        }
    }

    fn reject(&self, text: String, error: &str) {
        self.inbox.send(Msg::Agent(AgentEvent::PromptRejected {
            text,
            error: error.to_owned(),
        }));
    }

    fn forward(&self, cmd: Cmd) -> bool {
        if matches!(cmd, Cmd::Engine(_) | Cmd::Workbench(_) | Cmd::Checkpoint(_)) {
            return self.local_commands.send(cmd).is_ok();
        }
        self.connection
            .as_ref()
            .and_then(|current| current.commands.as_ref())
            .is_some_and(|commands| commands.send(cmd).is_ok())
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
    pub(crate) async fn run(
        mut self,
        commands: &mut Commands,
        first: Option<Cmd>,
        once: bool,
    ) -> (bool, Vec<LoopCmd>) {
        let mut first = first;
        let mut controls = Vec::new();
        let mut closed = false;
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
                    }
                }
                Cmd::Loop(command) => {
                    if matches!(command, LoopCmd::Abort) {
                        self.forward(Cmd::Cancel);
                    }
                    controls.push(command);
                }
                Cmd::Quit => {
                    closed = true;
                    break;
                }
                other => {
                    self.forward(other);
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
        let mut task = self.local_task;
        drop(self.local_commands);
        if tokio::time::timeout(CLOSE_GRACE, &mut task).await.is_err() {
            task.abort();
            let _ = task.await;
        }
        (!closed, controls)
    }

    pub(crate) fn spawn(self, mut commands: Commands) -> JoinHandle<Result<(), String>> {
        tokio::spawn(async move {
            self.run(&mut commands, None, false).await;
            Ok(())
        })
    }
}
