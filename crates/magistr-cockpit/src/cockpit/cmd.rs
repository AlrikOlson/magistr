//! What `update` asks the runtime to do.

use std::time::Duration;

use super::birth_cmd::BirthCmd;
use super::checkpoint_cmd::CheckpointCmd;
use super::engine_cmd::EngineCmd;
use super::kill_cmd::KillCmd;
use super::launch_action::LaunchAction;
use super::loop_cmd::LoopCmd;
use super::shelf_cmd::ShelfCmd;
use super::workbench_cmd::WorkbenchCmd;

/// A side effect for the runtime, returned by [`update`](super::update)
/// instead of performed. `Quit`, `Announce`, `Animate` and `Still` the
/// runtime handles itself; the rest go to whoever took the runtime's
/// commands — the session producer, or the loop driver sitting in front
/// of it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Cmd {
    /// Leave the terminal session and stop the event loop.
    Quit,
    /// Send a turn to the agent.
    Prompt(String),
    /// Request roadmap changes without implementing a work item.
    RoadmapPrompt(String),
    /// Cancel the pending or running roadmap request.
    CancelRoadmapPrompt,
    /// Ask the agent to stop the turn in flight.
    Cancel,
    /// Answer a permission request with one of its options.
    Permission {
        /// The request, as [`AgentEvent::PermissionRequest`] numbered it.
        ///
        /// [`AgentEvent::PermissionRequest`]: magistr_application::sessions::AgentEvent::PermissionRequest
        id: u64,
        /// The chosen option's id.
        option_id: String,
    },
    /// A standing constraint the reader wrote at a Moment — the
    /// conditional approval's second half ("allow, stop if other rows
    /// change"), on the ask it was written on.
    ///
    /// A command rather than model state alone because the constraint
    /// has to outlive the card: the cockpit records it on the charter
    /// so the reader can see what they agreed to, and hands the text to
    /// whoever owns the session's policy, which writes it as a rule
    /// over the ask's scope so the *next* ask there is decided by it.
    /// Sent *before* the answer that closes the ask,
    /// so the policy still holds the request the words are about. A
    /// `⇧A` carries no text: the scope it covers is the request's own,
    /// and the policy reads it off the answer.
    Constraint {
        /// The request, as [`AgentEvent::PermissionRequest`] numbered it.
        ///
        /// [`AgentEvent::PermissionRequest`]: magistr_application::sessions::AgentEvent::PermissionRequest
        id: u64,
        /// The reader's words.
        condition: String,
    },
    /// The reader accepted the charter's offer for the scope `key`:
    /// whoever owns the session's policy writes the
    /// always-allow the record supports. The rail's row is already
    /// noted by the time this is sent.
    Delegate(String),
    /// A decision for the iteration loop.
    Loop(LoopCmd),
    /// Start work in place — one item or the loop — in a session whose
    /// host can run it ([`Model::with_hosted_work`](super::Model::with_hosted_work)).
    /// The host answers a start it cannot make with
    /// [`Msg::CommandFailed`](super::Msg::CommandFailed), in words.
    Launch(LaunchAction),
    /// A held kill completed: the session turns a terminal into
    /// `terminal/kill` on the ACP transport; the loop driver in front
    /// of it turns a gate into a cancel on that gate's own child. Sent
    /// once, after the half-second hold — the model has already marked
    /// the screen killed by the time this leaves.
    Kill(KillCmd),
    /// Text for the reader's clipboard — a pane's stripped transcript.
    /// The surface serves it itself, like
    /// `Announce`: a terminal writes it as OSC 52 beside the frame, a
    /// window hands it to the system clipboard; it is never forwarded
    /// to a producer.
    Clipboard(String),
    /// An edit card's button: a request for the workbench port, not
    /// the agent.
    Workbench(WorkbenchCmd),
    /// A trace card's button: a request for the engine port, not the
    /// agent.
    Engine(EngineCmd),
    /// A checkpoint verb: a request for the checkpoint port, not the
    /// agent — the auto-save before a risky permission, and `/restore`
    /// and `/fork` on the ledger.
    Checkpoint(CheckpointCmd),
    /// Something on screen animates and wants a step every so often —
    /// the header's spinner while the turn streams. The runtime keeps
    /// its frame interval alive at this period and sends
    /// [`Msg::Tick`](super::Msg::Tick) each time one is owed; it is
    /// never forwarded to a producer. Re-declaring the period already
    /// in force changes nothing, so the core may say it on every
    /// message.
    Animate(Duration),
    /// Nothing animates any more: the runtime stops its tick, and an
    /// otherwise idle cockpit goes back to painting nothing at all.
    Still,
    /// A line for the screen-reader channel — a phase change or a new
    /// Needs-you card. The runtime writes it itself (like `Quit`), as
    /// an OSC 9 notification beside the frame; it is never forwarded
    /// to a producer.
    Announce(String),
    /// Redial the session's transport — the bad day's `Reconnect`
    /// (view 14). The runtime that owns a transport redials it; the
    /// engines' broadcast feed redials itself, so a runtime with
    /// nothing else to dial lets it pass.
    Reconnect,
    /// A shipped chunk's deliverables: collect them
    /// when a Done chunk is opened on a window, or write them as files
    /// on \[Export\]. The entry point that owns the project directory
    /// serves it as a job and posts the shelf back through the inbox;
    /// a consumer with no directory to read lets it pass.
    Shelf(ShelfCmd),
    /// A project being born: read the
    /// ground, draft the proposal, write the roadmap. The entry point
    /// that owns the file system serves it as a job and posts the
    /// answer back through the inbox; a consumer with no folder to
    /// read lets it pass.
    Birth(BirthCmd),
    /// Open the project in this directory in the same window. The host
    /// admits it before it settles the current project, answers with
    /// [`Msg::Rehosted`](super::Msg::Rehosted), and refuses in words
    /// through [`Msg::CommandFailed`](super::Msg::CommandFailed) with the
    /// current project still open.
    OpenProject(String),
}

impl Cmd {
    /// Report a closed surface-to-host connection with the unsent action intact.
    #[must_use]
    pub fn undelivered(self) -> super::Msg {
        super::Msg::CommandFailed {
            cmd: self,
            error: "The session connection closed. Reopen the session and try again.".into(),
        }
    }

    /// Return a refused effect through its semantic owner. A refusal is not a
    /// provider turn ending and cannot manufacture successful project evidence.
    pub(super) fn failed(self, model: &mut super::Model, error: String) -> Vec<Self> {
        use super::{CheckpointEvent, Msg, WorkbenchEvent};
        let message = match self {
            Self::Prompt(text) | Self::RoadmapPrompt(text) => {
                Msg::Agent(magistr_application::sessions::AgentEvent::PromptRejected {
                    text,
                    error,
                })
            }
            Self::Workbench(cmd) => Msg::Workbench(WorkbenchEvent::Failed { cmd, error }),
            Self::Engine(cmd) => Msg::Engine(crate::engine::EngineEvent::PortFailed {
                what: crate::engine::describe(&cmd),
                cmd,
                error,
            }),
            Self::Checkpoint(cmd) => {
                let (id, what) = crate::checkpoint::describe(&cmd);
                Msg::Checkpoint(CheckpointEvent::Failed {
                    id: Some(id),
                    what,
                    error,
                })
            }
            Self::OpenProject(dir) => return super::project_switch::refused(model, &dir, &error),
            other => {
                let action = match other {
                    Self::Cancel | Self::CancelRoadmapPrompt => {
                        if model.agent_activity.active && model.agent_activity.status == "Stopping"
                        {
                            model.agent_activity.status = "Stop failed";
                        }
                        "Magistr could not stop the response."
                    }
                    Self::Permission { id, .. } => {
                        super::decision::delivery_failed(model, id);
                        "Your answer was not sent."
                    }
                    Self::Constraint { .. } => "Your condition was not saved.",
                    Self::Delegate(_) => "Your permission setting was not saved.",
                    Self::Loop(_) => "Magistr could not change the current work.",
                    Self::Launch(_) => "Magistr could not start the work.",
                    Self::Kill(target) => {
                        if let Some(term) = model.terminals.term_mut(target.id()) {
                            term.kill_failed();
                        }
                        "Magistr could not stop the command."
                    }
                    Self::Reconnect => "Magistr could not reconnect.",
                    Self::Shelf(_) => "Magistr could not open or export the report.",
                    Self::Birth(_) => "Magistr could not set up the project.",
                    Self::Quit
                    | Self::Animate(_)
                    | Self::Still
                    | Self::Announce(_)
                    | Self::Clipboard(_) => "Magistr could not perform the action.",
                    Self::Prompt(_)
                    | Self::RoadmapPrompt(_)
                    | Self::Workbench(_)
                    | Self::Engine(_)
                    | Self::Checkpoint(_)
                    | Self::OpenProject(_) => {
                        unreachable!("project and prompt failures are handled above")
                    }
                };
                let text = format!("{action} {error}");
                model.session.turn.push_error(&text);
                return vec![Self::Announce(text)];
            }
        };
        super::update(model, message)
    }
}
