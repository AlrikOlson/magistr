//! Channel-based engine client for loop mode.
//!
//! Wraps a `tokio::sync::mpsc::UnboundedReceiver<Result<AgentMessage>>` as an
//! `EngineClient`. Used when the `AgentSession` runs in a spawned task and
//! messages flow through a channel (e.g., `magistr next-loop` iterations).
//!
//! This client is receive-only: `send_command` and `send_config_change` are
//! no-ops because the session lives in a separate task without a command channel.

use tokio::sync::mpsc;

use super::client::EngineClient;
use super::error::EngineClientError;
use crate::agent::message::AgentMessage;
use crate::agent::session::AcpSessionState;
use crate::error::Result;

/// Engine client backed by a message channel.
///
/// Receive-only implementation for loop mode where the `AgentSession` is
/// owned by a spawned task. Commands (follow-up, config) are not supported
/// because the session runs autonomously per iteration.
pub struct ChannelEngineClient {
    rx: mpsc::UnboundedReceiver<Result<AgentMessage>>,
}

impl ChannelEngineClient {
    /// Create a new channel engine client from an unbounded receiver.
    #[must_use]
    pub fn new(rx: mpsc::UnboundedReceiver<Result<AgentMessage>>) -> Self {
        Self { rx }
    }
}

impl EngineClient for ChannelEngineClient {
    async fn next_message(
        &mut self,
    ) -> Option<std::result::Result<AgentMessage, EngineClientError>> {
        self.rx
            .recv()
            .await
            .map(|r| r.map_err(EngineClientError::from))
    }

    async fn send_command(&self, _text: &str) -> std::result::Result<(), EngineClientError> {
        // Channel mode: session runs autonomously, no command channel available.
        Ok(())
    }

    async fn query_state(&self) -> std::result::Result<Option<AcpSessionState>, EngineClientError> {
        Ok(None)
    }

    fn cancel(&mut self) {
        // Channel is receive-only; the spawned task handles its own lifecycle.
        self.rx.close();
    }

    fn abort(&mut self) {
        self.rx.close();
    }

    async fn send_config_change(
        &self,
        _config_id: String,
        _value_id: String,
    ) -> std::result::Result<(), EngineClientError> {
        // Channel mode: no config change support.
        Ok(())
    }
}

#[cfg(test)]
#[path = "channel.tests.rs"]
mod tests;
