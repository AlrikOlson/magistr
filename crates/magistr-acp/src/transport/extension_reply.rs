//! Reply destinations for extensions, with one completion even on task loss.

use std::sync::{Arc, Mutex};

use agent_client_protocol::{Agent, ConnectionTo, UntypedMessage};
use serde_json::Value;
use tokio::sync::{mpsc, oneshot};

use crate::{AcpError, AcpEvent, Result};

/// An extension is either answered directly or observed in the event FIFO.
pub(crate) enum ExtensionReply {
    Direct(oneshot::Sender<Result<Value>>),
    Observed(u64),
}

impl ExtensionReply {
    pub(super) fn request(
        self,
        cx: &ConnectionTo<Agent>,
        method: String,
        params: Value,
        event_tx: mpsc::UnboundedSender<Result<AcpEvent>>,
    ) {
        let completion = Arc::new(Completion {
            reply: Mutex::new(Some(self)),
            event_tx,
            method,
        });
        let request = match UntypedMessage::new(&completion.method, params) {
            Ok(request) => request,
            Err(error) => {
                completion.finish(Err(AcpError::transport(format!(
                    "ACP {} could not be encoded: {error}",
                    completion.method
                ))));
                return;
            }
        };
        // Register immediately: ACP 2.0 awaits notification handlers before
        // routing a response. Its ordered callback also holds dispatch until
        // this bounded enqueue finishes when registered before routing. Never
        // await consumer acknowledgement here: it could need later traffic.
        let registered = cx.send_request(request).on_receiving_result({
            let completion = Arc::clone(&completion);
            async move |result| {
                completion.finish(result.map_err(|error| {
                    AcpError::transport(format!("ACP {} failed: {error}", completion.method))
                }));
                Ok(())
            }
        });
        if let Err(error) = registered {
            completion.finish(Err(AcpError::transport(format!(
                "ACP {} could not register its response task: {error}",
                completion.method
            ))));
        }
    }

    fn finish(self, event_tx: &mpsc::UnboundedSender<Result<AcpEvent>>, result: Result<Value>) {
        match self {
            Self::Direct(response_tx) => {
                let _ = response_tx.send(result);
            }
            Self::Observed(request_id) => {
                let _ = event_tx.send(Ok(AcpEvent::ExtensionResponse {
                    request_id,
                    result: result.map_err(|error| error.to_string()),
                }));
            }
        }
    }
}

// Retain a slot until callback registration returns so a registration failure
// can report its actual error. The callback owns the other reference; dropping
// an undelivered SDK task settles the slot rather than losing an observed id.
struct Completion {
    reply: Mutex<Option<ExtensionReply>>,
    event_tx: mpsc::UnboundedSender<Result<AcpEvent>>,
    method: String,
}

impl Completion {
    fn finish(&self, result: Result<Value>) {
        let reply = self
            .reply
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(reply) = reply {
            reply.finish(&self.event_tx, result);
        }
    }
}

impl Drop for Completion {
    fn drop(&mut self) {
        self.finish(Err(AcpError::transport(format!(
            "ACP {} connection ended before its response was delivered",
            self.method
        ))));
    }
}
