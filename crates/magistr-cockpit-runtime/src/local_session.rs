//! Session attachment for local project controls before an agent is connected.

use magistr_application::sessions::AgentEvent;
use magistr_application::sessions::SessionHandle;
use std::{future::Future, pin::Pin};

/// Owns no provider; the cockpit relay still serves its project ports.
pub(crate) struct LocalSession;

impl SessionHandle for LocalSession {
    fn submit_turn(&mut self, _: &str) -> Result<(), String> {
        Err("Connect to Magistr before sending a message.".into())
    }
    fn rename(&mut self, _: &str) -> Result<(), String> {
        Err("Naming is unavailable in this session.".into())
    }
    fn configure(&mut self, _: &str, _: &str) -> Result<(), String> {
        Err("Agent choices are unavailable in this session.".into())
    }
    fn cancel_turn(&mut self) -> Result<(), String> {
        Err("No agent is connected. Send a message to connect, then try again.".into())
    }
    fn answer_permission(&mut self, _: u64, _: &str) -> Result<(), String> {
        Err("No agent is connected. Send a message to connect, then try again.".into())
    }
    fn constrain_permission(&mut self, _: u64, _: &str) -> Result<(), String> {
        Err("No agent is connected. Send a message to connect, then try again.".into())
    }
    fn delegate_permission(&mut self, _: &str) -> Result<(), String> {
        Err("No agent is connected. Send a message to connect, then try again.".into())
    }
    fn kill_terminal(&mut self, _: &str) -> Result<(), String> {
        Err("No agent is connected. Send a message to connect, then try again.".into())
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
