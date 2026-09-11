//! The native workspace tools over MCP (chunk arch-14c10): a thin rmcp
//! adapter over [`magistr_workspace::service::WorkspaceService`].
//!
//! Everything the retained server promised its clients is served here and
//! nothing more: the eight schema-defined tools plus `workbench_status`,
//! namespaced plugin tools after the core names, schemas verbatim, the
//! envelope as every tool's `outputSchema` (so `structuredContent` and the
//! JSON text carry the same sealed envelope), per-request envelope
//! negotiation and session identity from `_meta`, the MRTR confirm flow
//! with its token fallback, the request's own cancellation into the worker,
//! the `index/build` task and one root span per call. Statelessness is
//! preserved for queries and envelopes. Each connection now holds a fallback
//! session identity; explicit `_meta` identities receive fresh owner admission.
//!
//! This module negotiates the wire without reaching into service stores.
//! `magistr workspace serve` admits one native workspace owner or attaches
//! another Unix MCP connection through the shared owner transport. Workflow
//! serving remains a separate subcommand and production launcher cutover is
//! deferred until the remaining workspace lifecycle acceptance is complete.
mod arguments;
mod connection;
mod dispatch;
mod elicitation;
#[cfg(unix)]
mod endpoint;
mod serve;
mod service;
mod session_lifetime;
mod tasks;
mod tool_surface;

pub use serve::run_workspace_stdio_server;
pub use service::WorkspaceMcpService;
