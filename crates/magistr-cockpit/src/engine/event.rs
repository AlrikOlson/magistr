//! What the feed — or the cockpit's own engine port — reports.

use magistr_workflow_contracts::{Delivery, FeedStatus, NativeEvent, NativeSnapshot};

use crate::cockpit::EngineCmd;

/// One thing the native store side said, as
/// [`Msg::Engine`](crate::cockpit::Msg::Engine) carries it: the live
/// feed's attachment, its typed events with how each was delivered, the
/// store read whole, and the answers of the
/// [`EnginePort`](super::EnginePort) — the in-process engines broadcast
/// nothing, so a cockpit-made pin or checkpoint comes back this way
/// rather than on the socket.
// `Event` carries a whole record (see `NativeEvent`), consumed as an
// invalidation when it leaves the inbox. Only validated owner reads are
// retained, so boxing the transient event would add an allocation per frame.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq)]
pub enum EngineEvent {
    /// The feed's attachment to the broadcast socket changed.
    Feed(FeedStatus),
    /// An engine changed something: one typed event, and where it stood in
    /// its publisher's sequence when the feed took it in.
    Event {
        /// What changed.
        event: NativeEvent,
        /// How it arrived — in order, a duplicate, after a gap, from a new
        /// owner, or from this process's own writer.
        delivery: Delivery,
    },
    /// The read coordinator's independent observations from all four owners.
    Seed(NativeSnapshot),
    /// Invalidate retained work records and ask the session's read coordinator
    /// to validate and publish all owners.
    Refresh,
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

impl EngineEvent {
    /// An event this process's own writer answered with — the port's
    /// chunk or signal, the loop driver's gate bytes — which has no
    /// publisher and nothing to sequence.
    #[must_use]
    pub const fn local(event: NativeEvent) -> Self {
        Self::Event {
            event,
            delivery: Delivery::Local,
        }
    }
}
