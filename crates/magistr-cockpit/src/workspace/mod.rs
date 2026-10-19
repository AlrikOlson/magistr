//! The workspace behind the cockpit's journal model, as a port.
//!
//! The cockpit never talks to the MCP transport itself: `update` answers a
//! click or a palette command with [`Cmd::Workspace`], whoever consumes
//! the runtime's commands calls [`apply`] on a [`WorkspacePort`], and
//! the resulting [`WorkspaceEvent`] comes back through the inbox as
//! [`Msg::Workspace`]. The harness fakes the port
//! ([`FakeWorkspace`](crate::harness::FakeWorkspace)); the session
//! applies an injected port. The CLI host supplies the concrete MCP adapter.
//!
//! [`Cmd::Workspace`]: crate::cockpit::Cmd::Workspace
//! [`Msg::Workspace`]: crate::cockpit::Msg::Workspace
//! [`WorkspaceEvent`]: crate::cockpit::WorkspaceEvent

pub mod apply;
pub mod describe;
pub mod journal_entry;
pub mod port;
pub mod run_row;

pub use crate::workspace::apply::apply;
pub use crate::workspace::describe::describe;
pub use crate::workspace::journal_entry::JournalEntry;
pub use port::WorkspacePort;
pub use run_row::RunRow;
