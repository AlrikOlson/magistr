//! How the driver gets a connected agent.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use magistr_acp::AcpTransport;
use magistr_application::effects::SourceRoute;

use crate::agent::{AgentConfig, ResolvedSessionSpec};
use crate::error::Result;

type Connect = dyn Fn(
        AgentConfig,
    ) -> Pin<Box<dyn Future<Output = Result<(AcpTransport, ResolvedSessionSpec)>> + Send>>
    + Send
    + Sync;

/// Turns an [`AgentConfig`] into a connected [`AcpTransport`] and its frozen setup — the one
/// seam between the loop driver and a process. [`Connector::live`]
/// spawns the real agent with the host's source route; a test hands in a
/// closure that connects the transport over a channel to an in-process fake.
#[derive(Clone)]
pub struct Connector {
    connection: Connection,
    observation: Option<Arc<crate::application::session_observation::SessionObservation>>,
}

#[derive(Clone)]
enum Connection {
    Live(SourceRoute),
    Custom(Arc<Connect>),
}

impl Connector {
    /// A connector from `connect`.
    pub fn new<F, Fut>(connect: F) -> Self
    where
        F: Fn(AgentConfig) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(AcpTransport, ResolvedSessionSpec)>> + Send + 'static,
    {
        Self {
            connection: Connection::Custom(Arc::new(move |config| Box::pin(connect(config)))),
            observation: None,
        }
    }

    /// Resolve setup once, spawn the agent and complete the handshake; the
    /// session's authorized source writes take `source` — the host's
    /// admitted workspace, or the refusal a host without one holds.
    #[must_use]
    pub fn live(source: SourceRoute) -> Self {
        Self {
            connection: Connection::Live(source),
            observation: None,
        }
    }

    pub(crate) fn observing(
        mut self,
        observation: Option<Arc<crate::application::session_observation::SessionObservation>>,
    ) -> Self {
        self.observation = observation;
        self
    }

    /// Bind durable and retained state before the supervisor can admit an opening turn.
    /// Raw connections, including judges, do not activate a displayed conversation.
    pub(crate) async fn start_session(
        &self,
        transport: AcpTransport,
        spec: ResolvedSessionSpec,
        options: magistr_application::sessions::SessionOptions,
    ) -> Result<crate::agent::cockpit_session::CockpitSession> {
        let history = spec.history();
        history.bind_transport(&transport);
        if let Some(observation) = &self.observation {
            observation
                .bind_history(history.clone())
                .await
                .map_err(crate::error::MagistrError::Other)?;
            observation
                .activate_context(&history.id())
                .map_err(crate::error::MagistrError::Other)?;
        }
        let (policy, _) = spec.into_session_policy();
        crate::agent::cockpit_session::CockpitSession::start_owned(
            transport, options, policy, history,
        )
    }

    /// Connect without a persisted loop operation (single sessions and fixtures).
    pub async fn connect(
        &self,
        config: AgentConfig,
    ) -> Result<(AcpTransport, ResolvedSessionSpec)> {
        self.connect_checkpointed(config, || Ok(())).await
    }

    /// Connect for `config`.
    ///
    /// # Errors
    ///
    /// Whatever the connection failed with.
    pub async fn connect_checkpointed(
        &self,
        config: AgentConfig,
        before_spawn: impl FnOnce() -> Result<()>,
    ) -> Result<(AcpTransport, ResolvedSessionSpec)> {
        match &self.connection {
            Connection::Live(source) => {
                let spec = config.resolve()?.with_source_route(source.clone());
                let transport = spec.connect_transport_checkpointed(before_spawn).await?;
                Ok((transport, spec))
            }
            Connection::Custom(connect) => {
                before_spawn()?;
                connect(config).await
            }
        }
    }
}

impl std::fmt::Debug for Connector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Connector")
    }
}
