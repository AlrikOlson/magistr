//! Magistr's ACP (Agent Client Protocol) transport.
//!
//! [`AcpTransport`] spawns an ACP agent (`magistr acp-agent`, `copilot --acp`),
//! runs the `initialize` → `session/new` | `session/load` handshake, and then
//! drives prompts, cancellation and config changes over the connection while
//! answering the agent's own requests — permission prompts, file reads and
//! writes, and terminals.
//!
//! ## Architecture
//!
//! Built on `agent-client-protocol` 2.0's role/handler model. The connection is
//! one `Send` future: `Client.builder()` with one `on_receive_*` handler per
//! agent→client method, connected over the child's stdio with [`Lines`], and
//! `tokio::spawn`ed. The command loop that sends `PromptRequest`s lives in the
//! `connect_with` foreground closure, which runs *outside* the dispatch loop —
//! so it may `block_task()` on responses. Handlers that have to wait (a
//! permission prompt in the interface, a terminal's exit) `cx.spawn` and respond from
//! the task, because a handler holds the dispatch loop while it runs.
//!
//! ```text
//! ┌──────────────────────────────────────────┐
//! │  AcpTransport (Send)                     │
//! │  connect / prompt / cancel / close       │
//! └──────────┬───────────────────────────────┘
//!            │ mpsc commands · mpsc events · mpsc permissions
//! ┌──────────┴───────────────────────────────┐
//! │  tokio task: Client.builder()…connect_with│
//! │   ├ handlers: permission, fs, terminal   │
//! │   └ foreground: handshake + command loop │
//! │  subprocess: the ACP agent (stdio)       │
//! └──────────────────────────────────────────┘
//! ```
//!
//! Protocol **v1** only (`schema::v1`, `ProtocolVersion::V1`); v2 is a draft
//! and stays off until it stabilises.
//!
//! [`Lines`]: agent_client_protocol::Lines

#![warn(missing_docs)]

pub mod capabilities;

mod error;
mod event;
mod handlers;
mod permission;
mod result;
mod session_mode;
mod stderr_tail;
/// Scripted protocol peers for cross-crate session tests.
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
mod transport;

pub use error::AcpError;
pub use event::AcpEvent;
// The escape stripper behind the agent's terminals, so Magistr's gate
// runner strips its gates' transcripts the same way (the pty pair itself
// is `magistr_process::Pty`).
pub use handlers::strip_escapes::EscapeStripper;
pub use permission::{
    AcpPermissionOption, AcpPermissionOptionKind, AcpPermissionRequest, AcpPermissionResponse,
};
pub use result::Result;
pub use session_mode::AcpSessionMode;
pub use transport::{AcpSender, AcpTransport};
