//! How the driver gets a connected agent.

use std::future::Future;
use std::pin::Pin;

use magistr_acp::AcpTransport;

use crate::agent::{AgentConfig, connect_transport};
use crate::error::Result;

type Connect =
    dyn Fn(AgentConfig) -> Pin<Box<dyn Future<Output = Result<AcpTransport>> + Send>> + Send + Sync;

/// Turns an [`AgentConfig`] into a connected [`AcpTransport`] — the one
/// seam between the loop driver and a process. [`Connector::live`]
/// spawns the real agent; a test hands in a closure that connects the
/// transport over a channel to an in-process fake.
pub struct Connector {
    connect: Box<Connect>,
}

impl Connector {
    /// A connector from `connect`.
    pub fn new<F, Fut>(connect: F) -> Self
    where
        F: Fn(AgentConfig) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<AcpTransport>> + Send + 'static,
    {
        Self {
            connect: Box::new(move |config| Box::pin(connect(config))),
        }
    }

    /// The real thing: [`connect_transport`] — detect the runtime, spawn
    /// the agent, complete the handshake.
    #[must_use]
    pub fn live() -> Self {
        Self::new(|config| async move { connect_transport(&config).await })
    }

    /// Connect for `config`.
    ///
    /// # Errors
    ///
    /// Whatever the connection failed with.
    pub async fn connect(&self, config: AgentConfig) -> Result<AcpTransport> {
        (self.connect)(config).await
    }
}

impl std::fmt::Debug for Connector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Connector")
    }
}
