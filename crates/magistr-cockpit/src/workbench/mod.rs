//! The workbench behind the cockpit's journal model, as a port.
//!
//! The cockpit never talks to workbench-mcp itself: `update` answers a
//! click or a palette command with [`Cmd::Workbench`], whoever consumes
//! the runtime's commands calls [`apply`] on a [`WorkbenchPort`], and
//! the resulting [`WorkbenchEvent`] comes back through the inbox as
//! [`Msg::Workbench`]. The harness fakes the port
//! ([`FakeWorkbench`](crate::harness::FakeWorkbench)); the session
//! applies an injected port. The CLI host supplies the concrete MCP adapter.
//!
//! [`Cmd::Workbench`]: crate::cockpit::Cmd::Workbench
//! [`Msg::Workbench`]: crate::cockpit::Msg::Workbench
//! [`WorkbenchEvent`]: crate::cockpit::WorkbenchEvent

pub mod apply;
pub mod describe;
pub mod journal_entry;
pub mod port;
pub mod run_row;

pub use apply::apply;
pub use describe::describe;
pub use journal_entry::JournalEntry;
pub use port::WorkbenchPort;
pub use run_row::RunRow;
