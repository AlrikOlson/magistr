//! One public MCP connection over the admitted workflow and working-copy owners.
mod access;
pub use access::Access;
mod catalog;
mod check;
mod client;
mod composition;
mod service;

pub use check::check_connection;
pub use service::run_stdio_server;

pub(crate) use catalog::DIRECT;

/// Whether the operation is exposed by the unified connection to MCP clients.
pub(crate) fn is_public_tool(name: &str) -> bool {
    DIRECT.iter().any(|(_, public, _)| *public == name) || matches!(name, "discover" | "execute")
}
