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
