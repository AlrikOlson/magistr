//! Interactive attachment to the host's shared resident session supervisor.

use std::future::Future;
use std::pin::Pin;

use magistr_acp::AcpTransport;
use magistr_application::sessions::SessionHandle;
use magistr_cockpit::agent::{AgentEvent, SessionOptions, SessionPolicy};

use super::ResolvedSessionSpec;
use super::observation::{ObservationCapacity, ObservationReceiver};
use super::session_supervisor::{SessionControl, SessionSupervisor, SessionSupervisorSnapshot};
use crate::error::Result;

/// The cockpit's command and observation attachment; owns no transport loop.
pub struct CockpitSession {
    supervisor: SessionSupervisor,
    events: ObservationReceiver<AgentEvent>,
}

impl CockpitSession {
    /// Connect the frozen host setup and hand policy and transport to the owner.
    pub async fn start(spec: ResolvedSessionSpec) -> Result<Self> {
        let transport = spec.connect_transport().await?;
        let options = SessionOptions {
            opening_prompt: (!spec.prompt().trim().is_empty()).then(|| spec.prompt().to_owned()),
            // Resolved transport setup has already validated model selection.
            model: None,
            max_turns: spec.max_turns(),
        };
        let (policy, _) = spec.into_permission_policy();
        Self::start_connected(transport, options, Box::new(policy))
    }

    /// Adopt an already connected transport, including real channel fixtures.
    pub fn start_connected(
        transport: AcpTransport,
        options: SessionOptions,
        policy: Box<dyn SessionPolicy>,
    ) -> Result<Self> {
        let (supervisor, events) =
            SessionSupervisor::start_interactive(transport, options, policy)?;
        Ok(Self { supervisor, events })
    }

    /// [`CockpitSession::start_connected`] under an explicit observation
    /// bound, for a fixture that floods the owner.
    pub fn start_connected_bounded(
        transport: AcpTransport,
        options: SessionOptions,
        policy: Box<dyn SessionPolicy>,
        capacity: ObservationCapacity,
    ) -> Result<Self> {
        let (supervisor, events) =
            SessionSupervisor::start_interactive_bounded(transport, options, policy, capacity)?;
        Ok(Self { supervisor, events })
    }

    /// Wind the session down the way an interrupt does; see
    /// [`SessionSupervisor::interrupt`].
    pub fn interrupt(&self) -> Result<()> {
        self.supervisor.interrupt()
    }

    /// Consistent lifecycle facts shared with headless observation.
    #[must_use]
    pub fn snapshot(&self) -> SessionSupervisorSnapshot {
        self.supervisor.snapshot()
    }

    /// Narrow host-only goal operations for the loop driver.
    #[must_use]
    pub fn control(&self) -> SessionControl {
        self.supervisor.control()
    }
}

impl SessionHandle for CockpitSession {
    type Event = AgentEvent;

    fn submit_turn(&mut self, text: &str) -> std::result::Result<(), String> {
        self.supervisor
            .send_followup(text)
            .map_err(|error| error.to_string())
    }
    fn cancel_turn(&mut self) -> std::result::Result<(), String> {
        self.supervisor
            .cancel_turn()
            .map_err(|error| error.to_string())
    }
    fn answer_permission(&mut self, id: u64, option_id: &str) -> std::result::Result<(), String> {
        self.supervisor
            .answer_permission(id, option_id)
            .map_err(|error| error.to_string())
    }
    fn constrain_permission(
        &mut self,
        id: u64,
        condition: &str,
    ) -> std::result::Result<(), String> {
        self.supervisor
            .constrain_permission(id, condition)
            .map_err(|error| error.to_string())
    }
    fn delegate_permission(&mut self, key: &str) -> std::result::Result<(), String> {
        self.supervisor
            .delegate_permission(key)
            .map_err(|error| error.to_string())
    }
    fn kill_terminal(&mut self, id: &str) -> std::result::Result<(), String> {
        self.supervisor
            .kill_terminal(id)
            .map_err(|error| error.to_string())
    }
    fn detach_client(&mut self) {
        self.supervisor.detach_client();
        // Detaching ends observation: buffered events belong to no client.
        self.events.close();
    }
    fn next_event(&mut self) -> Pin<Box<dyn Future<Output = Option<Self::Event>> + Send + '_>> {
        Box::pin(self.events.recv())
    }
    fn terminate_session(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = std::result::Result<(), String>> + Send + '_>> {
        Box::pin(async {
            self.supervisor
                .terminate_session()
                .await
                .map_err(|error| error.to_string())
        })
    }
}
