//! Background transport task that exclusively owns the transport.
//!
//! Demuxes incoming messages: notifications go to an `mpsc` channel,
//! responses are routed to `oneshot` senders keyed by request ID.
//! Commands flow from `TransportHandle` to the task via an `mpsc` channel.
//! This eliminates mutex contention between `next_message()` and `request()`
//! and prevents notification loss during request/response correlation.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use magistr_protocol::Transport;
use magistr_protocol::jsonrpc::{Message, Notification, RequestId};
use magistr_protocol::transport::TransportError;
use tokio::sync::{Mutex as TokioMutex, mpsc, oneshot};
use tokio::task::JoinHandle;

use super::error::EngineClientError;

/// Command sent from `TransportHandle` to the background transport task.
enum TransportCommand {
    /// Send a JSON-RPC request and await the matching response.
    Request {
        message: Message,
        request_id: RequestId,
        response_tx: oneshot::Sender<Result<serde_json::Value, EngineClientError>>,
    },
    /// Establish the transport connection.
    Connect {
        response_tx: oneshot::Sender<Result<(), EngineClientError>>,
    },
    /// Disconnect the transport.
    Disconnect {
        response_tx: oneshot::Sender<Result<(), EngineClientError>>,
    },
}

/// Handle to a background transport task.
///
/// Provides an async API for sending requests, receiving notifications,
/// and managing the transport connection — all without holding a mutex
/// across await points.
pub struct TransportHandle {
    /// Channel to send commands to the background task.
    cmd_tx: Option<mpsc::UnboundedSender<TransportCommand>>,
    /// Channel to receive notifications from the background task.
    notification_rx: mpsc::UnboundedReceiver<Result<Notification, EngineClientError>>,
    /// Whether the transport is currently connected.
    connected: Arc<AtomicBool>,
    /// Timestamp of the last successful operation.
    last_healthy: Arc<TokioMutex<Option<Instant>>>,
    /// Handle to the background task (for cleanup).
    task_handle: Option<JoinHandle<()>>,
}

impl TransportHandle {
    /// Spawn a background task that exclusively owns the transport.
    ///
    /// Returns a handle for sending commands and receiving notifications.
    pub fn spawn(transport: impl Transport + 'static) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (notification_tx, notification_rx) = mpsc::unbounded_channel();
        let connected = Arc::new(AtomicBool::new(false));
        let last_healthy = Arc::new(TokioMutex::new(None));

        let task_connected = Arc::clone(&connected);
        let task_last_healthy = Arc::clone(&last_healthy);

        let task_handle = tokio::spawn(transport_loop(
            transport,
            cmd_rx,
            notification_tx,
            task_connected,
            task_last_healthy,
        ));

        Self {
            cmd_tx: Some(cmd_tx),
            notification_rx,
            connected,
            last_healthy,
            task_handle: Some(task_handle),
        }
    }

    /// Establish the transport connection.
    pub async fn connect(&self) -> Result<(), EngineClientError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.send_cmd(TransportCommand::Connect { response_tx })?;
        response_rx
            .await
            .map_err(|_| EngineClientError::ConnectionLost("transport task exited".to_owned()))?
    }

    /// Disconnect the transport.
    pub async fn disconnect(&self) -> Result<(), EngineClientError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.send_cmd(TransportCommand::Disconnect { response_tx })?;
        response_rx
            .await
            .map_err(|_| EngineClientError::ConnectionLost("transport task exited".to_owned()))?
    }

    /// Send a JSON-RPC request and await the matching response.
    pub async fn send_request(
        &self,
        message: Message,
        request_id: RequestId,
    ) -> Result<serde_json::Value, EngineClientError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.send_cmd(TransportCommand::Request {
            message,
            request_id,
            response_tx,
        })?;
        response_rx
            .await
            .map_err(|_| EngineClientError::ConnectionLost("transport task exited".to_owned()))?
    }

    /// Receive the next notification from the transport.
    ///
    /// Returns `None` when the transport is closed and all buffered
    /// notifications have been consumed.
    pub async fn next_notification(&mut self) -> Option<Result<Notification, EngineClientError>> {
        self.notification_rx.recv().await
    }

    /// Whether the transport is currently connected.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    /// Time elapsed since the last successful operation.
    ///
    /// Returns `Duration::MAX` if no successful operation has occurred.
    pub fn time_since_last_healthy(&self) -> std::time::Duration {
        match self.last_healthy.try_lock() {
            Ok(guard) => guard.map_or(std::time::Duration::MAX, |t| t.elapsed()),
            Err(_) => std::time::Duration::MAX,
        }
    }

    /// Shut down the background task.
    ///
    /// Drops the command channel (causing the task to exit) and aborts the
    /// task handle if it hasn't finished yet.
    pub fn shutdown(&mut self) {
        // Drop the command sender to signal the task to exit.
        self.cmd_tx.take();
        // Abort the task if it's still running.
        if let Some(handle) = self.task_handle.take() {
            handle.abort();
        }
        self.connected.store(false, Ordering::SeqCst);
    }

    /// Send a command to the background task.
    fn send_cmd(&self, cmd: TransportCommand) -> Result<(), EngineClientError> {
        self.cmd_tx
            .as_ref()
            .ok_or_else(|| EngineClientError::ConnectionLost("transport shut down".to_owned()))?
            .send(cmd)
            .map_err(|_| EngineClientError::ConnectionLost("transport task exited".to_owned()))
    }
}

impl Drop for TransportHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Background task that exclusively owns the transport.
///
/// Reads incoming messages and demuxes them: responses are routed to the
/// matching pending request via `oneshot`, notifications are forwarded to
/// the notification channel. Commands from the handle are processed as they
/// arrive.
async fn transport_loop(
    mut transport: impl Transport + 'static,
    mut cmd_rx: mpsc::UnboundedReceiver<TransportCommand>,
    notification_tx: mpsc::UnboundedSender<Result<Notification, EngineClientError>>,
    connected: Arc<AtomicBool>,
    last_healthy: Arc<TokioMutex<Option<Instant>>>,
) {
    let mut pending: HashMap<
        RequestId,
        oneshot::Sender<Result<serde_json::Value, EngineClientError>>,
    > = HashMap::new();

    loop {
        // Only read from transport when connected; otherwise just wait for commands.
        if connected.load(Ordering::SeqCst) {
            tokio::select! {
                biased;

                cmd = cmd_rx.recv() => {
                    match cmd {
                        Some(cmd) => handle_command(
                            cmd,
                            &mut transport,
                            &mut pending,
                            &connected,
                            &last_healthy,
                        ).await,
                        None => break, // Handle dropped
                    }
                }

                msg = transport.recv_message() => {
                    match msg {
                        Ok(message) => {
                            handle_incoming_message(
                                message,
                                &mut pending,
                                &notification_tx,
                                &last_healthy,
                            ).await;
                        }
                        Err(TransportError::ConnectionClosed) => {
                            connected.store(false, Ordering::SeqCst);
                            // Fail all pending requests.
                            fail_all_pending(
                                &mut pending,
                                "connection closed",
                            );
                            // Signal notification stream that connection is closed.
                            let _ = notification_tx.send(Err(
                                EngineClientError::ConnectionLost("connection closed".to_owned()),
                            ));
                            break;
                        }
                        Err(e) => {
                            connected.store(false, Ordering::SeqCst);
                            fail_all_pending(&mut pending, &e.to_string());
                            let _ = notification_tx.send(Err(
                                EngineClientError::ConnectionLost(e.to_string()),
                            ));
                            break;
                        }
                    }
                }
            }
        } else {
            // Not connected — only process commands (connect, or queued requests
            // that will fail because we're disconnected).
            match cmd_rx.recv().await {
                Some(cmd) => {
                    handle_command(cmd, &mut transport, &mut pending, &connected, &last_healthy)
                        .await;
                }
                None => break,
            }
        }
    }
}

/// Process a command from the handle.
async fn handle_command(
    cmd: TransportCommand,
    transport: &mut (impl Transport + ?Sized),
    pending: &mut HashMap<RequestId, oneshot::Sender<Result<serde_json::Value, EngineClientError>>>,
    connected: &AtomicBool,
    last_healthy: &TokioMutex<Option<Instant>>,
) {
    match cmd {
        TransportCommand::Connect { response_tx } => {
            let result = transport
                .connect()
                .await
                .map_err(|e| EngineClientError::ConnectionLost(e.to_string()));
            if result.is_ok() {
                connected.store(true, Ordering::SeqCst);
                if let Ok(mut healthy) = last_healthy.try_lock() {
                    *healthy = Some(Instant::now());
                }
            }
            let _ = response_tx.send(result);
        }
        TransportCommand::Disconnect { response_tx } => {
            let result = transport
                .disconnect()
                .await
                .map_err(|e| EngineClientError::ConnectionLost(e.to_string()));
            connected.store(false, Ordering::SeqCst);
            let _ = response_tx.send(result);
        }
        TransportCommand::Request {
            message,
            request_id,
            response_tx,
        } => {
            if !connected.load(Ordering::SeqCst) {
                let _ = response_tx.send(Err(EngineClientError::ConnectionLost(
                    "not connected".to_owned(),
                )));
                return;
            }
            match transport.send_message(&message).await {
                Ok(()) => {
                    pending.insert(request_id, response_tx);
                }
                Err(e) => {
                    connected.store(false, Ordering::SeqCst);
                    let _ = response_tx.send(Err(EngineClientError::ConnectionLost(e.to_string())));
                }
            }
        }
    }
}

/// Route an incoming message to the appropriate destination.
async fn handle_incoming_message(
    message: Message,
    pending: &mut HashMap<RequestId, oneshot::Sender<Result<serde_json::Value, EngineClientError>>>,
    notification_tx: &mpsc::UnboundedSender<Result<Notification, EngineClientError>>,
    last_healthy: &TokioMutex<Option<Instant>>,
) {
    match message {
        Message::Response(resp) => {
            if let Some(tx) = pending.remove(&resp.id) {
                if let Ok(mut healthy) = last_healthy.try_lock() {
                    *healthy = Some(Instant::now());
                }
                let _ = tx.send(Ok(resp.result));
            }
            // Non-matching responses are dropped (shouldn't happen in practice).
        }
        Message::Error(err) => {
            if let Some(id) = &err.id
                && let Some(tx) = pending.remove(id)
            {
                let _ = tx.send(Err(EngineClientError::Protocol(err.error.message)));
            }
        }
        Message::Notification(notification) => {
            if let Ok(mut healthy) = last_healthy.try_lock() {
                *healthy = Some(Instant::now());
            }
            let _ = notification_tx.send(Ok(notification));
        }
        Message::Request(_) => {
            // Server-initiated requests are not expected; ignore.
        }
    }
}

/// Fail all pending requests with the given error message.
fn fail_all_pending(
    pending: &mut HashMap<RequestId, oneshot::Sender<Result<serde_json::Value, EngineClientError>>>,
    reason: &str,
) {
    for (_, tx) in pending.drain() {
        let _ = tx.send(Err(EngineClientError::ConnectionLost(reason.to_owned())));
    }
}

#[cfg(test)]
#[path = "transport_task.tests.rs"]
mod tests;
