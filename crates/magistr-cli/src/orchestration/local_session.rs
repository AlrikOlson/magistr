//! Session attachment for local project controls before an agent is connected.

use magistr_application::sessions::SessionHandle;
use magistr_cockpit::agent::AgentEvent;
use std::{future::Future, pin::Pin};

/// Owns no provider; the cockpit relay still serves its project ports.
pub(crate) struct LocalSession;

impl SessionHandle for LocalSession {
    type Event = AgentEvent;
    fn submit_turn(&mut self, _: &str) -> Result<(), String> {
        Err("Connect to Magistr before sending a message.".into())
    }
    fn cancel_turn(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn answer_permission(&mut self, _: u64, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn constrain_permission(&mut self, _: u64, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn delegate_permission(&mut self, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn kill_terminal(&mut self, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn detach_client(&mut self) {}
    fn next_event(&mut self) -> Pin<Box<dyn Future<Output = Option<AgentEvent>> + Send + '_>> {
        Box::pin(std::future::pending())
    }
    fn terminate_session(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }
}
