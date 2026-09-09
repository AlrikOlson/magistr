//! What the feed — or the cockpit's own engine port — reports.

use magistr_engine::{FeedStatus, NativeFrame};

use super::seed::StoreSeed;
use crate::cockpit::EngineCmd;

/// One thing the think-and-ship side said, as
/// [`Msg::Engine`](crate::cockpit::Msg::Engine) carries it: the live
/// feed's attachment, frames and seeds, and the answers of the
/// [`EnginePort`](super::EnginePort) — the in-process engines broadcast
/// nothing, so a cockpit-made pin or checkpoint comes back this way
/// rather than on the socket.
#[derive(Clone, Debug, PartialEq)]
pub enum EngineEvent {
    /// The feed's attachment to the broadcast socket changed.
    Feed(FeedStatus),
    /// An engine mutated something: one broadcast frame.
    Frame(NativeFrame),
    /// The store re-read whole: what the loop driver posts at each
    /// iteration start and what
    /// [`spawn_reseed`](super::spawn_reseed) posts on its timer, for
    /// state recorded while no feed was attached.
    Seed(StoreSeed),
    /// The port marked the step at the cockpit's ask.
    Pinned {
        /// The step that was marked.
        step_number: u32,
        /// The flag now in effect.
        pinned: bool,
    },
    /// The port took a trace checkpoint: the report, line by line.
    Checkpoint {
        /// The report's lines.
        lines: Vec<String>,
    },
    /// The port refused an ask: an error line for the stage.
    PortFailed {
        /// The refused command, so the dock can restore its answer.
        cmd: EngineCmd,
        /// What was asked (`pin 7`, `checkpoint`).
        what: String,
        /// The engine's reason.
        error: String,
    },
}
