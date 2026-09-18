//! Compatibility ACP facade backed by the reusable resident session supervisor.

use tracing::instrument;

use super::AgentConfig;
#[cfg(test)]
use super::convert_acp_event::convert_acp_event;
use super::observation::ObservationReceiver;
use super::session::AcpSessionState;
use super::session_supervisor::{SessionSupervisor, SessionSupervisorSnapshot};
use crate::agent::message::AgentMessage;
use crate::error::Result;

type BackendStart = (AcpBackend, ObservationReceiver<Result<AgentMessage>>);

/// ACP facade; the supervisor owns transport, policy, admission and residency.
pub struct AcpBackend {
    supervisor: SessionSupervisor,
}

impl AcpBackend {
    /// Resolve and start a session. The supervisor owns permission handling
    /// before any prompt is admitted.
    #[instrument(skip_all, name = "acp_backend_start")]
    pub async fn start(config: AgentConfig) -> Result<BackendStart> {
        Self::start_resolved(config.resolve()?).await
    }

    pub(crate) async fn start_resolved(spec: super::ResolvedSessionSpec) -> Result<BackendStart> {
        let (supervisor, messages) = SessionSupervisor::start(spec).await?;
        Ok((Self { supervisor }, messages))
    }

    pub(crate) async fn start_checkpointed(
        spec: super::ResolvedSessionSpec,
        before_spawn: impl FnOnce() -> Result<()>,
    ) -> Result<BackendStart> {
        let (supervisor, messages) =
            SessionSupervisor::start_checkpointed(spec, before_spawn).await?;
        Ok((Self { supervisor }, messages))
    }

    #[cfg(test)]
    async fn start_connected(
        prompt: &str,
        transport: magistr_acp::AcpTransport,
    ) -> Result<BackendStart> {
        let (supervisor, messages) =
            SessionSupervisor::start_connected(prompt, transport, None).await?;
        Ok((Self { supervisor }, messages))
    }

    /// Send a follow-up; busy sessions retain it in FIFO order.
    pub fn send_followup(&self, content: &str) -> Result<()> {
        self.supervisor.send_followup(content)
    }

    /// Change configuration between turns; asynchronous errors enter the message stream.
    pub fn send_config_change(&self, config_id: String, value_id: String) -> Result<()> {
        self.supervisor.send_config_change(config_id, value_id)
    }

    /// Stop residency immediately, preserving observed terminal facts.
    pub fn abort(&mut self) {
        self.supervisor.abort();
    }

    /// Legacy cancellation ends observation after the actual provider terminal response.
    pub fn cancel(&mut self) {
        self.supervisor.cancel();
    }

    /// Request cancellation while retaining the session for subsequent turns.
    pub fn cancel_turn(&self) -> Result<()> {
        self.supervisor.cancel_turn()
    }

    /// Wind the session down the way an interrupt does; see
    /// [`SessionSupervisor::interrupt`].
    pub fn interrupt(&self) -> Result<()> {
        self.supervisor.interrupt()
    }

    /// Stop residency and await transport cleanup.
    pub async fn terminate_session(&mut self) -> Result<()> {
        self.supervisor.terminate_session().await
    }

    /// Remove observation only; the provider remains resident.
    pub fn detach_client(&mut self) {
        self.supervisor.detach_client();
    }

    /// Read precise outcome, cancellation and connection facts.
    #[must_use]
    pub fn supervisor_snapshot(&self) -> SessionSupervisorSnapshot {
        self.supervisor.snapshot()
    }

    /// The provider process the supervisor registered, if any.
    #[must_use]
    pub fn provider_pid(&self) -> Option<u32> {
        self.supervisor.provider_pid()
    }

    /// List sessions between turns.
    pub async fn list_sessions(
        &self,
    ) -> Result<Vec<agent_client_protocol::schema::v1::SessionInfo>> {
        self.supervisor.list_sessions().await
    }

    /// Update an idle session configuration.
    pub async fn set_config_option(
        &self,
        config_id: String,
        value: agent_client_protocol::schema::v1::SessionConfigOptionValue,
    ) -> Result<Vec<agent_client_protocol::schema::v1::SessionConfigOption>> {
        self.supervisor.set_config_option(config_id, value).await
    }

    /// Compatibility lifecycle projection; use the snapshot for precise facts.
    pub fn session_state(&self) -> Option<AcpSessionState> {
        Some(self.supervisor.session_state())
    }

    /// Negotiated ACP capabilities after initialization.
    pub fn agent_capabilities(
        &self,
    ) -> Option<agent_client_protocol::schema::v1::AgentCapabilities> {
        self.supervisor.agent_capabilities()
    }

    /// Number of confirmed terminal prompt responses.
    #[must_use]
    pub fn turn_count(&self) -> u32 {
        self.supervisor.turn_count()
    }
}

#[cfg(test)]
#[path = "backend.tests.rs"]
mod tests;
