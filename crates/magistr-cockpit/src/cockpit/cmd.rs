//! What `update` asks the runtime to do.

use std::time::Duration;

use super::birth_cmd::BirthCmd;
use super::checkpoint_cmd::CheckpointCmd;
use super::engine_cmd::EngineCmd;
use super::kill_cmd::KillCmd;
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
        /// [`AgentEvent::PermissionRequest`]: crate::agent::AgentEvent::PermissionRequest
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
        /// [`AgentEvent::PermissionRequest`]: crate::agent::AgentEvent::PermissionRequest
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
}
