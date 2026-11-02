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
    /// \[Pin\] / \[Unpin\] on an expanded note.
    Pin {
        /// The note to mark.
        note: u32,
        /// The flag to set.
        pinned: bool,
    },
    /// \[Review\]: the log-wide metacognitive report.
    Review,
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
    /// \[Promote\] on a request: it becomes a plan concern.
    PromoteRequest {
        /// The request to promote.
        id: String,
    },
    /// \[Ignore\] on a request: dismissed for good.
    DismissRequest {
        /// The request to dismiss.
        id: String,
    },
    /// \[Snooze\] on a request: quiet for a while.
    SnoozeRequest {
        /// The request to snooze.
        id: String,
        /// How long it stays quiet.
        minutes: u32,
    },
}
