//! One task per session, running its turns in order.

use std::sync::Arc;
use std::time::Duration;

use agent_client_protocol::schema::v1::{PromptResponse, SessionId, StopReason};
use agent_client_protocol::{Client, ConnectionTo};
use tokio::sync::mpsc;

use crate::queued_turn::QueuedTurn;
use crate::session_registry::SessionRegistry;
use crate::{Provider, Turn};

/// How long a cancelled provider without a runtime acknowledgment contract may
/// keep running before its turn is dropped and answered `cancelled`. A provider
/// that ignores its token must not hold the session indefinitely.
const CANCEL_GRACE: Duration = Duration::from_secs(2);

/// Drain a session's queue until the session closes: each turn runs on the
/// provider, one at a time, and answers its own `session/prompt`.
///
/// Runs off the dispatch loop (`cx.spawn`), so the provider may wait on the
/// client — a permission answer — without deadlocking the connection.
pub(crate) async fn run_session_worker(
    provider: Arc<dyn Provider>,
    registry: SessionRegistry,
    session_id: SessionId,
    cx: ConnectionTo<Client>,
    mut queue: mpsc::UnboundedReceiver<QueuedTurn>,
) -> Result<(), agent_client_protocol::Error> {
    let requires_terminal_cancellation = provider.requires_terminal_cancellation();
    while let Some(QueuedTurn {
        request,
        responder,
        cancel,
    }) = queue.recv().await
    {
        if cancel.is_cancelled() {
            // Cancelled while waiting its turn: the provider never sees it.
            responder.respond(PromptResponse::new(StopReason::Cancelled))?;
            continue;
        }
        let model = registry.model_of(&session_id).ok().flatten();
        let turn = Turn::new(
            session_id.clone(),
            request.prompt,
            model,
            cancel.clone(),
            cx.clone(),
        );
        let overrun = async {
            cancel.cancelled().await;
            tokio::time::sleep(CANCEL_GRACE).await;
        };
        let outcome = tokio::select! {
            outcome = provider.prompt(turn) => outcome,
            () = overrun, if !requires_terminal_cancellation => {
                tracing::warn!(session = %session_id, "provider ignored the cancel; turn dropped");
                Ok(StopReason::Cancelled)
            }
        };
        // A provider with a runtime acknowledgment contract owns cancellation
        // completion: its refusal or timeout must reach ACP as an error, and
        // the next queued turn must wait until that bounded outcome arrives.
        // Other providers retain the worker's cancellation override and grace.
        let response = match outcome {
            _ if cancel.is_cancelled() && !requires_terminal_cancellation => {
                Ok(PromptResponse::new(StopReason::Cancelled))
            }
            Ok(stop) => Ok(PromptResponse::new(stop)),
            Err(e) => Err(e.into()),
        };
        responder.respond_with_result(response)?;
    }
    Ok(())
}
