//! The live session's presentation relay and local cockpit effects.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use magistr_application::sessions::SessionHandle;
use tokio::task::JoinHandle;

use super::event::AgentEvent;
use super::session_jobs::SessionJobs;
use super::tool_status::ToolStatus;
use crate::channel::{Commands, Inbox};
use crate::checkpoint::CheckpointPort;
use crate::cockpit::{CheckpointCmd, CheckpointEvent, EngineCmd, WorkbenchCmd, WorkbenchEvent};
use crate::cockpit::{Cmd, KillCmd, Msg};
use crate::engine::{EngineEvent, EnginePort};
use crate::workbench::WorkbenchPort;

/// A port the relay holds for its owned jobs: shared with the blocking
/// thread that runs the call, held by one relay, so never contended
/// beyond the jobs' own serialization on it.
type Shared<P> = Arc<Mutex<Box<P>>>;

/// Attach a session owner to the cockpit's events and commands.
///
/// The handle owns provider startup, turn admission, permission decisions and
/// cancellation. This relay stamps presentation durations, posts its events to
/// the inbox, and forwards session commands. Opening transcript events come
/// from the owner; later prompts are already echoed by the cockpit model.
/// Workbench, engine and checkpoint requests run as the relay's owned jobs
/// ([`SessionJobs`]) on their local ports, off this task and independently of
/// the provider; a request whose port is absent is refused by name rather
/// than answered with silence. Once the runtime ends, the jobs are fenced and
/// joined, then the session owner's cleanup is awaited.
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
        workbench: workbench.map(|port| Arc::new(Mutex::new(port))),
        engine: engine.map(|port| Arc::new(Mutex::new(port))),
        checkpoints: checkpoints.map(|port| Arc::new(Mutex::new(port))),
        jobs: SessionJobs::new(),
        tool_started: HashMap::new(),
        started: Instant::now(),
    };
    tokio::spawn(session.run(commands))
}

struct Session {
    handle: Box<dyn SessionHandle<Event = AgentEvent>>,
    inbox: Inbox,
    workbench: Option<Shared<dyn WorkbenchPort>>,
    engine: Option<Shared<dyn EnginePort>>,
    checkpoints: Option<Shared<dyn CheckpointPort>>,
    jobs: SessionJobs,
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
        // The fence closes before the owner is asked to end, so no job
        // answers on behalf of a session that is gone.
        let left = self.jobs.settle().await;
        if left > 0 {
            self.inbox.send(Msg::Agent(AgentEvent::Error(format!(
                "{left} owned job(s) were still running when the session ended; their answers are discarded"
            ))));
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
            Cmd::Prompt(text) => {
                if let Err(error) = self.handle.submit_turn(&text) {
                    self.inbox
                        .send(Msg::Agent(AgentEvent::PromptRejected { text, error }));
                }
                Ok(())
            }
            Cmd::RoadmapPrompt(text) => {
                self.inbox.send(Msg::Agent(AgentEvent::PromptRejected {
                    text,
                    error:
                        "This session cannot update the roadmap. Open a new session to try again."
                            .to_owned(),
                }));
                Ok(())
            }
            Cmd::CancelRoadmapPrompt => {
                self.inbox.send(Msg::RoadmapRequestCancelled);
                Ok(())
            }
            Cmd::Cancel => self.handle.cancel_turn(),
            Cmd::Permission { id, option_id } => self.handle.answer_permission(id, &option_id),
            Cmd::Constraint { id, condition } => self.handle.constrain_permission(id, &condition),
            Cmd::Delegate(key) => self.handle.delegate_permission(&key),
            Cmd::Kill(KillCmd::Terminal(id)) => self.handle.kill_terminal(&id),
            Cmd::Workbench(cmd) => {
                self.workbench(cmd);
                Ok(())
            }
            Cmd::Engine(cmd) => {
                self.engine(cmd);
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

    /// A workbench call is an owned job: a slow search must not pause the
    /// provider's events, and one asked of a session with no workbench is
    /// refused by name.
    fn workbench(&mut self, cmd: WorkbenchCmd) {
        let what = crate::workbench::describe(&cmd);
        let Some(port) = self.workbench.as_ref().map(Arc::clone) else {
            self.inbox.send(Msg::Workbench(WorkbenchEvent::Failed {
                cmd,
                error: "no workbench is attached to this session".to_owned(),
            }));
            return;
        };
        let asked = cmd.clone();
        if let Err(error) = self.jobs.run(&what, &self.inbox, move || {
            let mut port = port.lock().unwrap_or_else(PoisonError::into_inner);
            Msg::Workbench(crate::workbench::apply(port.as_mut(), &cmd))
        }) {
            self.inbox
                .send(Msg::Workbench(WorkbenchEvent::Failed { cmd: asked, error }));
        }
    }

    /// An engine call is an owned job too: the store is on disk, and a
    /// session without an engine port says so.
    fn engine(&mut self, cmd: EngineCmd) {
        let what = crate::engine::describe(&cmd);
        let Some(port) = self.engine.as_ref().map(Arc::clone) else {
            self.inbox.send(Msg::Engine(EngineEvent::PortFailed {
                cmd,
                what,
                error: "no engine port is attached to this session".to_owned(),
            }));
            return;
        };
        let refused = cmd.clone();
        if let Err(error) = self.jobs.run(&what, &self.inbox, move || {
            let mut port = port.lock().unwrap_or_else(PoisonError::into_inner);
            Msg::Engine(crate::engine::apply(port.as_mut(), &cmd))
        }) {
            self.inbox.send(Msg::Engine(EngineEvent::PortFailed {
                cmd: refused,
                what,
                error,
            }));
        }
    }

    /// Git and worktree operations must not pause incoming agent events;
    /// a session that holds no ledger refuses rather than dropping the ask.
    fn checkpoint(&mut self, cmd: CheckpointCmd) {
        let (id, what) = crate::checkpoint::describe(&cmd);
        let Some(port) = self.checkpoints.as_ref().map(Arc::clone) else {
            self.inbox.send(Msg::Checkpoint(CheckpointEvent::Failed {
                id: Some(id),
                what,
                error: "no checkpoint ledger is attached to this session".to_owned(),
            }));
            return;
        };
        if let Err(error) = self.jobs.run(&what, &self.inbox, move || {
            let mut port = port.lock().unwrap_or_else(PoisonError::into_inner);
            Msg::Checkpoint(crate::checkpoint::apply(port.as_mut(), &cmd))
        }) {
            self.inbox.send(Msg::Checkpoint(CheckpointEvent::Failed {
                id: Some(id),
                what,
                error,
            }));
        }
    }
}

#[cfg(test)]
#[path = "session.tests.rs"]
mod tests;
