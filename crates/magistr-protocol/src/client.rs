//! Typed SDK client for the magistr protocol.
//!
//! Wraps any [`Transport`] implementation with strongly-typed methods that
//! map 1:1 to the JSON-RPC protocol. This is the seed for the Rust SDK
//! client library — the VS Code extension's protocol client can be modeled
//! after this.
//!
//! # Example
//!
//! ```ignore
//! use magistr_protocol::client::MagistrClient;
//! use magistr_protocol::transport::TcpTransport;
//!
//! let transport = TcpTransport::connect_to("127.0.0.1:9257", Duration::from_secs(5)).await?;
//! let mut client = MagistrClient::new(transport);
//! client.connect().await?;
//!
//! let health = client.system_health().await?;
//! println!("Server status: {}", health.status);
//! ```

use std::sync::atomic::{AtomicI64, Ordering};

use crate::jsonrpc::{Message, Request};
use crate::methods::{self, method};
use crate::transport::{Transport, TransportError};

/// Error type for client operations.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    /// Transport-level error.
    #[error("transport error: {0}")]
    Transport(#[from] TransportError),
    /// Received a non-response message when expecting a response.
    #[error("unexpected message type (expected response)")]
    UnexpectedMessage,
    /// JSON-RPC error response from the server.
    #[error("server error {code}: {message}")]
    ServerError {
        code: i64,
        message: String,
        data: Option<serde_json::Value>,
    },
    /// Failed to deserialize the response result.
    #[error("deserialization error: {0}")]
    Deserialization(#[from] serde_json::Error),
}

/// Typed client for the magistr JSON-RPC protocol.
///
/// Provides strongly-typed methods for each protocol endpoint. Uses an
/// auto-incrementing request ID counter.
pub struct MagistrClient<T: Transport> {
    pub(crate) transport: T,
    next_id: AtomicI64,
}

impl<T: Transport> MagistrClient<T> {
    /// Create a new client wrapping the given transport.
    ///
    /// The transport is not connected — call [`connect`](Self::connect) first.
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            next_id: AtomicI64::new(1),
        }
    }

    /// Connect the underlying transport.
    pub async fn connect(&mut self) -> Result<(), TransportError> {
        self.transport.connect().await
    }

    /// Disconnect the underlying transport.
    pub async fn disconnect(&mut self) -> Result<(), TransportError> {
        self.transport.disconnect().await
    }

    /// Check whether the transport is connected.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.transport.is_connected()
    }

    // ── System methods ──────────────────────────────────────────────────

    /// Query server and protocol version.
    pub async fn system_version(&mut self) -> Result<methods::SystemVersionResult, ClientError> {
        self.call(method::SYSTEM_VERSION, None).await
    }

    /// Query server capabilities.
    pub async fn system_capabilities(
        &mut self,
    ) -> Result<methods::SystemCapabilitiesResult, ClientError> {
        self.call(method::SYSTEM_CAPABILITIES, None).await
    }

    /// Request server shutdown.
    pub async fn system_shutdown(&mut self) -> Result<methods::SystemShutdownResult, ClientError> {
        self.call(method::SYSTEM_SHUTDOWN, None).await
    }

    /// Query server health.
    pub async fn system_health(&mut self) -> Result<methods::SystemHealthResult, ClientError> {
        self.call(method::SYSTEM_HEALTH, None).await
    }

    /// Query server metrics.
    pub async fn system_metrics(&mut self) -> Result<methods::SystemMetricsResult, ClientError> {
        self.call(method::SYSTEM_METRICS, None).await
    }

    // ── Session methods ─────────────────────────────────────────────────

    /// Query session status (uses the server's default session).
    pub async fn session_status(&mut self) -> Result<methods::SessionStatusResult, ClientError> {
        self.call(method::SESSION_STATUS, None).await
    }

    /// List active sessions.
    pub async fn session_list(&mut self) -> Result<methods::SessionListResult, ClientError> {
        self.call(method::SESSION_LIST, None).await
    }

    // ── Workflow methods ────────────────────────────────────────────────

    /// Query current workflow phase.
    pub async fn workflow_phase(&mut self) -> Result<methods::WorkflowPhaseResult, ClientError> {
        self.call(method::WORKFLOW_PHASE, None).await
    }

    /// Query iteration state.
    pub async fn workflow_iteration(
        &mut self,
    ) -> Result<methods::WorkflowIterationResult, ClientError> {
        self.call(method::WORKFLOW_ITERATION, None).await
    }

    // ── Roadmap methods ─────────────────────────────────────────────────

    /// Query roadmap progress.
    pub async fn roadmap_status(&mut self) -> Result<methods::RoadmapStatusResult, ClientError> {
        self.call(method::ROADMAP_STATUS, None).await
    }

    // ── Cost methods ────────────────────────────────────────────────────

    /// Query cost status.
    pub async fn cost_status(&mut self) -> Result<methods::CostStatusResult, ClientError> {
        self.call(method::COST_STATUS, None).await
    }

    // ── Project methods ─────────────────────────────────────────────────

    /// Query project context.
    pub async fn project_context(&mut self) -> Result<methods::ProjectContextResult, ClientError> {
        self.call(method::PROJECT_CONTEXT, None).await
    }

    // ── Multi-project methods ───────────────────────────────────────────

    /// Register a project for multi-project serving.
    pub async fn register_project(
        &mut self,
        project_id: &str,
        project_dir: &str,
    ) -> Result<methods::SystemRegisterProjectResult, ClientError> {
        let params = serde_json::json!({
            "project_id": project_id,
            "project_dir": project_dir,
        });
        self.call(method::SYSTEM_REGISTER_PROJECT, Some(params))
            .await
    }

    /// Unregister a project.
    pub async fn unregister_project(
        &mut self,
        project_id: &str,
    ) -> Result<methods::SystemUnregisterProjectResult, ClientError> {
        let params = serde_json::json!({ "project_id": project_id });
        self.call(method::SYSTEM_UNREGISTER_PROJECT, Some(params))
            .await
    }

    /// List registered projects.
    pub async fn list_projects(
        &mut self,
    ) -> Result<methods::SystemListProjectsResult, ClientError> {
        self.call(method::SYSTEM_LIST_PROJECTS, None).await
    }

    // ── Raw call ────────────────────────────────────────────────────────

    /// Send a raw request and deserialize the typed response.
    async fn call<R: serde::de::DeserializeOwned>(
        &mut self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<R, ClientError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let request = Request::new(id, method, params);
        let msg = Message::Request(request);

        self.transport.send_message(&msg).await?;

        let response_msg = self.transport.recv_message().await?;
        match response_msg {
            Message::Response(resp) => Ok(serde_json::from_value(resp.result)?),
            Message::Error(err) => Err(ClientError::ServerError {
                code: err.error.code,
                message: err.error.message,
                data: err.error.data,
            }),
            _ => Err(ClientError::UnexpectedMessage),
        }
    }
}

#[cfg(test)]
#[path = "client.tests.rs"]
mod tests;
