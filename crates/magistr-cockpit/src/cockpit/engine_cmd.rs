//! What the cockpit asks of the think-and-ship engines.

use super::proposal_kind::ProposalKind;

/// An engine request from the cockpit, carried by
/// [`Cmd::Engine`](super::Cmd::Engine): the consumer runs it through an
/// [`EnginePort`](crate::engine::EnginePort) and posts the outcome back
/// as [`EngineEvent`](crate::engine::EngineEvent) — the cockpit's own
/// mutations never wait for the broadcast feed, which the in-process
/// engines do not write.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum EngineCmd {
    /// \[Pin\] / \[Unpin\] on an expanded step.
    Pin {
        /// The step to mark.
        step_number: u32,
        /// The flag to set.
        pinned: bool,
    },
    /// \[Checkpoint\]: the trace-wide metacognitive report.
    Checkpoint,
    /// \[Start\] on a roadmap chunk: mark it in progress.
    StartChunk {
        /// The chunk to start.
        id: String,
    },
    /// \[Complete\] on a roadmap chunk: mark it done.
    CompleteChunk {
        /// The chunk to complete.
        id: String,
    },
    /// Accept or reject an open proposal — from the dock's front card
    /// or the Roadmap tab's proposal list.
    ResolveProposal {
        /// The chunk the proposal is on.
        chunk: String,
        /// Which field it wants to change.
        kind: ProposalKind,
        /// Take the suggestion, or clear it.
        accept: bool,
    },
    /// \[Clear\] on a blocked chunk: lift its awaiting-human blocker.
    ClearBlocker {
        /// The blocked chunk.
        chunk: String,
    },
    /// \[Promote\] on a signal: it becomes a roadmap concern.
    PromoteSignal {
        /// The signal to promote.
        id: String,
    },
    /// \[Ignore\] on a signal: dismissed for good.
    IgnoreSignal {
        /// The signal to dismiss.
        id: String,
    },
    /// \[Snooze\] on a signal: quiet for a while.
    SnoozeSignal {
        /// The signal to snooze.
        id: String,
        /// How long it stays quiet.
        minutes: u32,
    },
}
