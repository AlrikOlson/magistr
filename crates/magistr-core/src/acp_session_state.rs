//! ACP session lifecycle state.
//!
//! Represents the formal lifecycle states defined by the Agent Client Protocol.
//! Both desktop and command-line consumers frontends use this to display session status.

/// ACP session lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcpSessionState {
    /// Session created but no prompt sent yet.
    Created,
    /// Agent is processing a prompt (thinking, executing tools).
    InProgress,
    /// Agent completed a turn and is awaiting the next prompt.
    Awaiting,
    /// Session completed normally.
    Completed,
    /// Session ended due to an error.
    Failed,
    /// Session was cancelled via graceful cancel request.
    Cancelled,
}

#[cfg(test)]
#[path = "acp_session_state.tests.rs"]
mod tests;
