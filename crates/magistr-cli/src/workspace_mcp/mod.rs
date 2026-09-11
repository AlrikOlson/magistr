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
//! structural: a request's answer is a function of the workspace and its
//! own `_meta`; nothing here holds per-session state.
//!
//! This module negotiates the wire. It never reaches past the service into
//! the stores, and `magistr mcp-serve` — the project workflow server — is
//! untouched: the native workspace server is its own subcommand,
//! `magistr workspace serve`, until owner cutover.
mod arguments;
mod dispatch;
mod elicitation;
mod serve;
mod service;
mod tasks;
mod tool_surface;

pub use serve::run_workspace_stdio_server;
pub use service::WorkspaceMcpService;
