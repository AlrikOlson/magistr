//! The checkpoint spine behind the rail's ledger, as a port.
//!
//! The cockpit never captures a session or runs git itself: `update`
//! answers a risky permission or a palette command with
//! [`Cmd::Checkpoint`], whoever consumes the runtime's commands calls
//! [`apply`] on a [`CheckpointPort`] — through [`detach`], on a blocking
//! thread, never on the task that relays the agent — and the resulting
//! [`CheckpointEvent`] comes back through the inbox as
//! [`Msg::Checkpoint`]. The harness fakes the port
//! ([`FakeCheckpoints`](crate::harness::FakeCheckpoints)); the real one
//! is the CLI's `orchestration::ledger::Checkpoints`, over the
//! engine handle that can resume a workflow and the git repository that
//! can open a worktree.
//!
//! The division is the same one `SessionSnapshot` draws: the cockpit
//! holds names and shows a chain, the adapter holds the state.
//!
//! [`Cmd::Checkpoint`]: crate::cockpit::Cmd::Checkpoint
//! [`Msg::Checkpoint`]: crate::cockpit::Msg::Checkpoint
//! [`CheckpointEvent`]: crate::cockpit::CheckpointEvent

pub mod apply;
pub mod detach;
pub mod port;

pub use apply::apply;
pub use detach::detach;
pub use port::CheckpointPort;
