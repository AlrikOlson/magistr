//! The workbench behind the cockpit's Changes tab, as a port.
//!
//! The cockpit never talks to workbench-mcp itself: `update` answers a
//! click or a palette command with [`Cmd::Workbench`], whoever consumes
//! the runtime's commands calls [`apply`] on a [`WorkbenchPort`], and
//! the resulting [`WorkbenchEvent`] comes back through the inbox as
//! [`Msg::Workbench`]. The harness fakes the port
//! ([`FakeWorkbench`](crate::harness::FakeWorkbench)); the session
//! applies an injected port, and [`McpWorkbench`] is that port over a
//! real workbench-mcp server, spoken to as MCP over its stdio.
//!
//! [`Cmd::Workbench`]: crate::cockpit::Cmd::Workbench
//! [`Msg::Workbench`]: crate::cockpit::Msg::Workbench
//! [`WorkbenchEvent`]: crate::cockpit::WorkbenchEvent

pub mod apply;
pub mod describe;
pub mod journal_entry;
pub mod mcp_client;
pub mod mcp_workbench;
pub mod port;
pub mod run_row;

pub use apply::apply;
pub use describe::describe;
pub use journal_entry::JournalEntry;
pub use mcp_client::McpClient;
pub use mcp_workbench::McpWorkbench;
pub use port::WorkbenchPort;
pub use run_row::RunRow;
