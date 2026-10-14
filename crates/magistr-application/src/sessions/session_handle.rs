//! Runtime-independent commands and observations for one resident agent session.

use super::AgentEvent;
use alloc::{boxed::Box, string::String};
use core::{future::Future, pin::Pin};

/// A client attachment to the application-owned session lifecycle.
///
/// The application owns the observation vocabulary; the host chooses the runtime.
/// Cancellation ends a turn only after the provider acknowledges it; detachment
/// ends observation while explicit termination releases the session and its
/// resources. Dropping the
/// owning handle also releases those resources, including pending decisions.
pub trait SessionHandle: Send {
    /// Admit input in FIFO order after any active turn.
    fn submit_turn(&mut self, text: &str) -> Result<(), String>;
    /// Assign the conversation's name through its history owner.
    fn rename(&mut self, name: &str) -> Result<(), String>;
    /// Request a value from the provider's currently advertised select options.
    fn configure(&mut self, id: &str, value: &str) -> Result<(), String>;
    /// Cancel active and queued input while retaining session residency.
    fn cancel_turn(&mut self) -> Result<(), String>;
    /// Answer a pending request using an option that request actually offered.
    fn answer_permission(&mut self, id: u64, option_id: &str) -> Result<(), String>;
    /// Attach a condition to a still-pending permission request.
    fn constrain_permission(&mut self, id: u64, condition: &str) -> Result<(), String>;
    /// Delegate a scope through the session policy.
    fn delegate_permission(&mut self, key: &str) -> Result<(), String>;
    /// Stop one terminal owned by the session.
    fn kill_terminal(&mut self, id: &str) -> Result<(), String>;
    /// Stop observing without terminating the resident provider.
    fn detach_client(&mut self);
    /// Observe the next event; `None` means the attachment has ended. Dropping
    /// a pending observation future must not consume or lose an event.
    fn next_event(&mut self) -> Pin<Box<dyn Future<Output = Option<AgentEvent>> + Send + '_>>;
    /// Terminate the resident session and await resource cleanup.
    fn terminate_session(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>>;
}
