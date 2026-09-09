//! Magistr's ACP (Agent Client Protocol) agent side.
//!
//! [`magistr-acp`](../magistr_acp/index.html) is the *client*: it spawns an
//! agent and drives the session. This crate is the *agent*: [`serve`] answers
//! `initialize`, `authenticate`, `session/new`, `session/load`,
//! `session/prompt`, `session/cancel` and `session/set_config_option`, relays
//! `session/request_permission` to the client, and emits every kind of
//! `session/update` — on behalf of a [`Provider`], which only knows how a
//! model is driven. `magistr acp-agent --provider <name>` serves it over
//! stdio ([`serve_stdio`]), so magistr spawns its own binary and never an
//! npm shim.
//!
//! ## Architecture
//!
//! Built on `agent-client-protocol` 2.0's role/handler model, the mirror of
//! the client crate: `Agent.builder()` with one `on_receive_*` handler per
//! client→agent method. A handler holds the dispatch loop while it runs, so
//! only the quick ones answer inline; opening a session (the provider may be
//! slow to start) and every turn run in spawned tasks, and a turn asks the
//! client for permission from its own task.
//!
//! ```text
//! ┌────────────────────────────────────────────────────┐
//! │  Agent.builder() — handlers (dispatch loop)         │
//! │   initialize · authenticate · set_config_option    │
//! │   session/new, session/load ─► spawn: open_session │
//! │   session/prompt ─► registry.enqueue               │
//! │   session/cancel ─► registry.cancel                │
//! └──────────┬─────────────────────────────────────────┘
//!            │ SessionRegistry: per session a queue + a cancellation
//! ┌──────────┴─────────────────────────────────────────┐
//! │  per-session worker (spawned): turns one at a time │
//! │   Turn ─► Provider::prompt                         │
//! │     message/thought/tool_call/usage/plan ─► client │
//! │     request_permission ◄─► client                  │
//! └────────────────────────────────────────────────────┘
//! ```
//!
//! Cancellation is the registry's one rule: a queued turn carries the
//! cancellation live when it was queued; `session/cancel` fires it and arms a
//! fresh one. The in-flight turn stops, turns queued before the cancel are
//! answered `cancelled` without reaching the provider, and a cancelled turn
//! is reported `cancelled` whatever the provider returned.
//!
//! Protocol **v1** only (`schema::v1`, `ProtocolVersion::V1`), the same pin as
//! the client crate.

#![warn(missing_docs)]

mod api;
mod claude;
mod codex;
mod error;
mod model_choice;
mod model_options;
mod provider;
mod queued_turn;
mod scripted;
mod serve;
mod serve_stdio;
mod session_entry;
mod session_opened;
mod session_registry;
mod session_start;
mod session_worker;
mod turn;

pub use api::{
    ApiProvider, ChatApi, ChatMessage, Gemini, ModelSpec, Ollama, OpenAi, Pricing, Role,
    StreamEvent,
};
pub use claude::ClaudeProvider;
pub use codex::{CodexProvider, ThreadRecord};
pub use error::AgentError;
pub use model_choice::ModelChoice;
pub use provider::Provider;
pub use scripted::ScriptedProvider;
pub use serve::serve;
pub use serve_stdio::serve_stdio;
pub use session_opened::SessionOpened;
pub use session_start::SessionStart;
pub use turn::Turn;
