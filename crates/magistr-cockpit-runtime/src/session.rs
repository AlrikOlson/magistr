//! The live session's presentation relay and local cockpit effects.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use magistr_application::sessions::SessionHandle;
use tokio::task::JoinHandle;

use super::session_jobs::SessionJobs;
use crate::channel::{Commands, Inbox};
use magistr_application::sessions::AgentEvent;
use magistr_application::sessions::ToolStatus;
use magistr_cockpit::checkpoint::CheckpointPort;
use magistr_cockpit::cockpit::{
    CheckpointCmd, CheckpointEvent, EngineCmd, WorkspaceCmd, WorkspaceEvent,
};
use magistr_cockpit::cockpit::{Cmd, KillCmd, Msg};
use magistr_cockpit::engine::{EngineEvent, EnginePort};
use magistr_cockpit::workspace::WorkspacePort;

/// A port the relay holds for its owned jobs: shared with the blocking
/// thread that runs the call, held by one relay, so never contended
/// beyond the jobs' own serialization on it.
type Shared<P> = Arc<Mutex<Box<P>>>;

/// Attach a session owner to the cockpit's events and commands.
///
/// The handle owns provider startup, turn admission, permission decisions and
/// cancellation. This relay stamps presentation durations, posts its events to
/// the inbox, and forwards session commands. Opening transcript events come
/// from the provider; accepted later inputs are published by this relay once.
/// Workspace, engine and checkpoint requests run as the relay's owned jobs
/// on their local ports, off this task and independently of
/// the provider; a request whose port is absent is refused by name rather
/// than answered with silence. Once the runtime ends, the jobs are fenced and
/// joined, then the session owner's cleanup is awaited. Aborting the relay also
/// fences job results, but cannot stop a running blocking operation.
///
/// The caller owns the returned task: close the command channel and await it
/// for graceful cleanup, or abort and await it. Dropping its handle detaches it.
pub fn spawn_session(
    handle: Box<dyn SessionHandle>,
    inbox: Inbox,
    commands: Commands,
    workspace: Option<Box<dyn WorkspacePort>>,
    engine: Option<Box<dyn EnginePort>>,
    checkpoints: Option<Box<dyn CheckpointPort>>,
) -> JoinHandle<Result<(), String>> {
    let inbox = match handle.conversation() {
        Some(conversation) => inbox.for_conversation(&conversation.id),
        None => inbox,
    };
    let session = Session {
        handle,
        inbox,
        workspace: workspace.map(|port| Arc::new(Mutex::new(port))),
        engine: engine.map(|port| Arc::new(Mutex::new(port))),
        checkpoints: checkpoints.map(|port| Arc::new(Mutex::new(port))),
        jobs: SessionJobs::new(),
        tool_started: HashMap::new(),
        started: Instant::now(),
        input_origin: InputOrigin::ProviderHistory,
    };
    tokio::spawn(session.run(commands))
}

struct Session {
    handle: Box<dyn SessionHandle>,
    inbox: Inbox,
    workspace: Option<Shared<dyn WorkspacePort>>,
    engine: Option<Shared<dyn EnginePort>>,
    checkpoints: Option<Shared<dyn CheckpointPort>>,
    jobs: SessionJobs,
    tool_started: HashMap<String, Instant>,
    started: Instant,
    input_origin: InputOrigin,
}

/// Initial/resumed history comes from the provider; after admission this relay
/// publishes application input and provider echoes cannot create a second row.
enum InputOrigin {
    ProviderHistory,
    Application,
}

impl Session {
    async fn run(mut self, mut commands: Commands) -> Result<(), String> {
        let mut observing = true;
        loop {
            tokio::select! {
                event = self.handle.next_event(), if observing => match event {
                    Some(mut event) => {
                        if matches!(self.input_origin, InputOrigin::Application) && matches!(event, AgentEvent::UserText(_)) { continue; }
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
        let conversation = self.handle.conversation();
        let Some(cmd) = self.inbox.command_for(
            cmd,
            conversation.as_ref().map(|context| context.id.as_str()),
        ) else {
            return;
        };
        let cmd = match cmd {
            Cmd::Workspace(cmd) => return self.workspace(cmd),
            Cmd::Engine(cmd) => return self.engine(cmd),
            Cmd::Checkpoint(cmd) => return self.checkpoint(cmd),
            Cmd::Prompt(text) => {
                match self.handle.submit_turn(&text) {
                    Ok(()) => {
                        self.input_origin = InputOrigin::Application;
                        self.inbox.send(Msg::InputAccepted(text.receipt()));
                    }
                    Err(error) => {
                        self.inbox.send(Msg::Agent(AgentEvent::PromptRejected {
                            input: text,
                            error,
                        }));
                    }
                }
                return;
            }
            Cmd::RoadmapPrompt(text) => {
                self.inbox.send(Msg::Agent(AgentEvent::PromptRejected {
                    input: text,
                    error:
                        "This session cannot update the roadmap. Open a new session to try again."
                            .into(),
                }));
                return;
            }
            cmd => cmd,
        };
        let result = match &cmd {
            Cmd::Rename(name) => self.handle.rename(name),
            Cmd::Configure { id, value } => self.handle.configure(id, value),
            Cmd::Cancel => self.handle.cancel_turn(),
            Cmd::Permission { id, option_id } => self.handle.answer_permission(*id, option_id),
            Cmd::Constraint { id, condition } => self.handle.constrain_permission(*id, condition),
            Cmd::Delegate(key) => self.handle.delegate_permission(key),
            Cmd::Kill(KillCmd::Terminal(id)) => self.handle.kill_terminal(id),
            // These need a different owner. Receiving them is not evidence
            // that their requested effect happened.
            _ => Err("This action is unavailable in this session.".into()),
        };
        if let Err(error) = result {
            self.inbox.send(Msg::CommandFailed { cmd, error });
        }
    }

    /// A workspace call is an owned job: a slow search must not pause the
    /// provider's events, and one asked of a session with no workspace is
    /// refused by name.
    fn workspace(&mut self, cmd: WorkspaceCmd) {
        let what = magistr_cockpit::workspace::describe(&cmd);
        let Some(port) = self.workspace.as_ref().map(Arc::clone) else {
            self.inbox.send(Msg::Workspace(WorkspaceEvent::Failed {
                cmd,
                error: "no workspace is attached to this session".to_owned(),
            }));
            return;
        };
        let asked = cmd.clone();
        if let Err(error) = self.jobs.run(&what, &self.inbox, move || {
            let mut port = port.lock().unwrap_or_else(PoisonError::into_inner);
            Msg::Workspace(magistr_cockpit::workspace::apply(port.as_mut(), &cmd))
        }) {
            self.inbox
                .send(Msg::Workspace(WorkspaceEvent::Failed { cmd: asked, error }));
        }
    }

    /// An engine call is an owned job too: the store is on disk, and a
    /// session without an engine port says so.
    fn engine(&mut self, cmd: EngineCmd) {
        let what = magistr_cockpit::engine::describe(&cmd);
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
            Msg::Engine(magistr_cockpit::engine::apply(port.as_mut(), &cmd))
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
        let (id, what) = magistr_cockpit::checkpoint::describe(&cmd);
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
            Msg::Checkpoint(magistr_cockpit::checkpoint::apply(port.as_mut(), &cmd))
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
