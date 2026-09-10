//! Permission requests are answered by the same owner that admits turns.

use std::sync::Mutex;

use magistr_acp::{AcpPermissionRequest, AcpPermissionResponse};
use tokio::sync::mpsc;

use super::{Inner, Policy};

pub(super) struct Permissions {
    receiver: Option<mpsc::UnboundedReceiver<AcpPermissionRequest>>,
    policy: Option<Policy>,
}

impl Permissions {
    pub(super) fn new(
        receiver: Option<mpsc::UnboundedReceiver<AcpPermissionRequest>>,
        policy: Option<Policy>,
    ) -> Self {
        Self { receiver, policy }
    }

    pub(super) async fn recv(&mut self) -> Option<AcpPermissionRequest> {
        match self.receiver.as_mut() {
            Some(receiver) => receiver.recv().await,
            None => std::future::pending().await,
        }
    }

    pub(super) fn answer(&mut self, request: Option<AcpPermissionRequest>, state: &Mutex<Inner>) {
        let Some(request) = request else {
            self.receiver = None;
            return;
        };
        // A stale request whose effect was cancelled must never mutate policy or
        // acquire a fresh grant after the next turn begins.
        if request.response_tx.is_closed() {
            return;
        }
        // Sending the reply and closing admission serialize through this lock.
        // Once cancel_turn returns, no old request can receive another grant.
        let inner = state.lock().expect("supervisor state lock poisoned");
        let response = if inner.closing
            || inner.snapshot.cancellation_requested
            || inner.snapshot.active_turn.is_none()
        {
            AcpPermissionResponse::Cancelled
        } else if let Some((policy, handling)) = self.policy.as_mut() {
            super::super::noninteractive_response::noninteractive_response(
                policy.as_mut(),
                &request,
                *handling,
            )
        } else {
            AcpPermissionResponse::Cancelled
        };
        let _ = request.response_tx.send(response);
    }

    pub(super) fn drain(&mut self) {
        if let Some(receiver) = self.receiver.as_mut() {
            while let Ok(request) = receiver.try_recv() {
                let _ = request.response_tx.send(AcpPermissionResponse::Cancelled);
            }
        }
    }

    pub(super) fn close(&mut self) {
        if let Some(receiver) = self.receiver.as_mut() {
            receiver.close();
        }
        self.drain();
        self.receiver = None;
    }
}

impl Drop for Permissions {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
#[path = "permissions.tests.rs"]
mod tests;
