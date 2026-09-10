//! The live session's presentation relay and local cockpit effects.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use magistr_application::sessions::SessionHandle;
use tokio::task::JoinHandle;

use super::event::AgentEvent;
use super::tool_status::ToolStatus;
use crate::channel::{Commands, Inbox};
use crate::checkpoint::CheckpointPort;
use crate::cockpit::CheckpointCmd;
use crate::cockpit::{Cmd, KillCmd, Msg};
use crate::engine::EnginePort;
use crate::workbench::WorkbenchPort;

/// Attach a session owner to the cockpit's events and commands.
///
/// The handle owns provider startup, turn admission, permission decisions and
/// cancellation. This relay stamps presentation durations, posts its events to
/// the inbox, and forwards session commands. Opening transcript events come
/// from the owner; later prompts are already echoed by the cockpit model.
/// Workbench, engine and checkpoint requests use their local ports independently
/// of the provider. Once the runtime ends, await the session owner's cleanup.
pub fn spawn_session(
    handle: Box<dyn SessionHandle<Event = AgentEvent>>,
    inbox: Inbox,
    commands: Commands,
    workbench: Option<Box<dyn WorkbenchPort>>,
    engine: Option<Box<dyn EnginePort>>,
    checkpoints: Option<Box<dyn CheckpointPort>>,
) -> JoinHandle<Result<(), String>> {
    let session = Session {
        handle,
        inbox,
        workbench,
        engine,
        checkpoints: checkpoints.map(|port| Arc::new(Mutex::new(port))),
        tool_started: HashMap::new(),
        started: Instant::now(),
    };
    tokio::spawn(session.run(commands))
}

struct Session {
    handle: Box<dyn SessionHandle<Event = AgentEvent>>,
    inbox: Inbox,
    workbench: Option<Box<dyn WorkbenchPort>>,
    engine: Option<Box<dyn EnginePort>>,
    /// Shared because checkpoint operations run off the relay task.
    checkpoints: Option<Arc<Mutex<Box<dyn CheckpointPort>>>>,
    tool_started: HashMap<String, Instant>,
    started: Instant,
}

impl Session {
    async fn run(mut self, mut commands: Commands) -> Result<(), String> {
        let mut observing = true;
        loop {
            tokio::select! {
                event = self.handle.next_event(), if observing => match event {
                    Some(mut event) => {
                        self.stamp(&mut event);
                        if !self.inbox.send(Msg::Agent(event)) {
                            break;
                        }
                    }
                    // Failure evidence belongs to the owner. A closed event
                    // stream cannot manufacture a successful terminal response.
                    // Keep local project operations available until the UI ends.
                    None => observing = false,
                },
                cmd = commands.next() => match cmd {
                    Some(cmd) => self.command(cmd),
                    None => break,
                },
            }
        }
        if let Err(error) = self.handle.terminate_session().await {
            self.inbox
                .send(Msg::Agent(AgentEvent::Error(error.clone())));
            return Err(error);
        }
        Ok(())
    }

    /// Supply wall-clock measurements as data so the model remains pure.
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
            AgentEvent::Usage { elapsed_secs, .. } => {
                *elapsed_secs = Some(self.started.elapsed().as_secs());
            }
            _ => {}
        }
    }

    fn command(&mut self, cmd: Cmd) {
        let result = match cmd {
            Cmd::Prompt(text) => self.handle.submit_turn(&text),
            Cmd::Cancel => self.handle.cancel_turn(),
            Cmd::Permission { id, option_id } => self.handle.answer_permission(id, &option_id),
            Cmd::Constraint { id, condition } => self.handle.constrain_permission(id, &condition),
            Cmd::Delegate(key) => self.handle.delegate_permission(&key),
            Cmd::Kill(KillCmd::Terminal(id)) => self.handle.kill_terminal(&id),
            Cmd::Workbench(cmd) => {
                if let Some(port) = self.workbench.as_deref_mut() {
                    let event = crate::workbench::apply(port, &cmd);
                    self.inbox.send(Msg::Workbench(event));
                }
                Ok(())
            }
            Cmd::Engine(cmd) => {
                if let Some(port) = self.engine.as_deref_mut() {
                    let event = crate::engine::apply(port, &cmd);
                    self.inbox.send(Msg::Engine(event));
                }
                Ok(())
            }
            Cmd::Checkpoint(cmd) => {
                self.checkpoint(cmd);
                Ok(())
            }
            // The runtime and loop driver serve these before commands reach
            // the session. A gate terminal belongs to the driver too.
            Cmd::Kill(KillCmd::Gate(_))
            | Cmd::Quit
            | Cmd::Loop(_)
            | Cmd::Shelf(_)
            | Cmd::Birth(_)
            | Cmd::Announce(_)
            | Cmd::Clipboard(_)
            | Cmd::Reconnect
            | Cmd::Animate(_)
            | Cmd::Still => Ok(()),
        };
        if let Err(error) = result {
            self.inbox.send(Msg::Agent(AgentEvent::Error(error)));
        }
    }

    /// Git and worktree operations must not pause incoming agent events.
    fn checkpoint(&mut self, cmd: CheckpointCmd) {
        if let Some(port) = &self.checkpoints {
            crate::checkpoint::detach(port, cmd, &self.inbox);
        }
    }
}

#[cfg(test)]
#[path = "session.tests.rs"]
mod tests;
