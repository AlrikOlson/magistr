//! Engine client abstraction layer.
//!
//! Provides a trait-based interface (`EngineClient`) for communicating with
//! agent engines, decoupling the TUI and command handlers from the concrete
//! backend implementation.
//!
//! ## Implementations
//!
//! - `LocalEngineClient` — wraps `AgentSession` in-process (zero overhead)
//! - `RemoteEngineClient` — connects to magistr-server via JSON-RPC (E4a.2)

pub mod channel;
pub mod client;
pub mod error;
pub mod local;
pub mod reconnect;
pub mod remote;
mod result;
pub mod transport_task;

pub use channel::ChannelEngineClient;
pub use client::EngineClient;
pub use error::EngineClientError;
pub use local::LocalEngineClient;
pub use reconnect::ReconnectionPolicy;
pub use remote::RemoteEngineClient;
pub use result::Result;
