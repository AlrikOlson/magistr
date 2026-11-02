//! What the cockpit asks of the magistr engines.

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
    /// \[Start\] on a plan task: mark it in progress.
    StartTask {
        /// The task to start.
        id: String,
    },
    /// \[Complete\] on a plan task: mark it done.
    CompleteTask {
        /// The task to complete.
        id: String,
    },
    /// Accept or reject an open proposal — from the dock's front card
    /// or the Plan tab's proposal list.
    ResolveProposal {
        /// The task the proposal is on.
        task: String,
        /// Which field it wants to change.
        kind: ProposalKind,
        /// Take the suggestion, or clear it.
        accept: bool,
    },
    /// \[Clear\] on a blocked task: lift its awaiting-human blocker.
    ClearBlocker {
        /// The blocked task.
        task: String,
    },
    /// \[Promote\] on a signal: it becomes a plan concern.
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
