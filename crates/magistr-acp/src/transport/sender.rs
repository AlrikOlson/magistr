//! A handle for sending on a session whose transport someone else owns.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::{mpsc, oneshot};

use super::command::AcpCommand;
use crate::handlers::effects::EffectBridge;
use crate::{AcpError, Result};

/// The sending half of an [`AcpTransport`](super::AcpTransport), cloned
/// off it with [`AcpTransport::sender`](super::AcpTransport::sender):
/// a turn, a cancel, or a vendor extension request, on the same command
/// channel the transport itself uses. A loop driver keeps one while the
/// cockpit's session owns the transport and its event stream.
#[derive(Clone)]
pub struct AcpSender {
    ready: Arc<AtomicBool>,
    cmd_tx: mpsc::UnboundedSender<AcpCommand>,
    effects: Option<Arc<EffectBridge>>,
}

impl AcpSender {
    pub(crate) fn new(
        ready: Arc<AtomicBool>,
        cmd_tx: mpsc::UnboundedSender<AcpCommand>,
        effects: Option<Arc<EffectBridge>>,
    ) -> Self {
        Self {
            ready,
            cmd_tx,
            effects,
        }
    }

    /// Whether the session is live.
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::SeqCst)
    }

    pub(crate) fn send(&self, what: &str, command: AcpCommand) -> Result<()> {
        if !self.is_ready() {
            return Err(AcpError::transport(format!(
                "ACP transport is not ready — cannot {what}"
            )));
        }
        if matches!(command, AcpCommand::Cancel | AcpCommand::Close)
            && let Some(effects) = &self.effects
        {
            effects.cancel();
        }
        self.cmd_tx.send(command).map_err(|_| {
            AcpError::transport(format!("ACP connection has shut down — cannot {what}"))
        })
    }

    /// Send one user turn. The turn's updates and its `PromptResponse`
    /// arrive on the transport's event channel.
    pub fn prompt(&self, text: &str) -> Result<()> {
        self.send("prompt", AcpCommand::Prompt(text.to_string()))
    }

    /// Ask the agent to stop the turn in flight (`session/cancel`).
    pub fn cancel(&self) -> Result<()> {
        self.send("cancel", AcpCommand::Cancel)
    }

    /// Send a vendor extension request — `method` must be the agent's
    /// own `_`-prefixed name, taken from what it advertised — and get
    /// the receiver its raw JSON answer will land on. The request is
    /// issued and answered off the connection's command loop, mid-turn
    /// included, so the caller decides when (and whether) to wait: an
    /// extension may not answer until the turn it started has ended.
    ///
    /// # Errors
    ///
    /// When the session is not live. A failure of the request itself
    /// arrives on the receiver.
    pub fn extension(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<oneshot::Receiver<Result<serde_json::Value>>> {
        let (response_tx, response_rx) = oneshot::channel();
        self.send(
            "send an extension request",
            AcpCommand::Extension {
                method: method.to_string(),
                params,
                response_tx,
            },
        )?;
        Ok(response_rx)
    }
}

impl std::fmt::Debug for AcpSender {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AcpSender")
            .field("ready", &self.is_ready())
            .finish_non_exhaustive()
    }
}
