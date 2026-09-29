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
//! `magistr workspace serve` — and the production alias `magistr mcp-launch
//! workbench-mcp` — is the bridge in [`bridge`]: it admits one native
//! workspace owner or attaches to it through the shared owner transport,
//! stamps a durable client identity, and reconnects on its next request when
//! the owner goes away. Workflow serving remains a separate subcommand.
mod arguments;
mod bridge;
mod bridge_link;
mod bridge_session;
mod capability;
mod connection;
mod dispatch;
mod elicitation;
#[cfg(any(unix, windows))]
mod endpoint;
mod serve;
mod service;
mod session_lifetime;
mod tasks;
mod tool_surface;

#[cfg(any(unix, windows))]
pub(crate) use endpoint::serve as serve_owner_endpoint;
pub(crate) use serve::open_options;
pub use serve::run_workspace_stdio_server;
pub use service::WorkspaceMcpService;
