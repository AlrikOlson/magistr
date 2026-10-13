//! Goal extensions settle in the owner's ordered provider event stream.

use std::sync::{Arc, Mutex};

use magistr_acp::{AcpError, AcpTransport};
use magistr_application::sessions::{AgentEvent, TurnEnd};
use tokio::sync::oneshot;

use super::{Inner, SessionResidency, TurnOutcome, acp_error};

pub(super) struct Goal {
    pub(super) turn_id: u64,
    reply: oneshot::Sender<magistr_acp::Result<serde_json::Value>>,
}

impl Goal {
    pub(super) fn begin(
        transport: &AcpTransport,
        state: &Arc<Mutex<Inner>>,
        method: String,
        params: serde_json::Value,
        turn_id: u64,
        reply: oneshot::Sender<magistr_acp::Result<serde_json::Value>>,
    ) -> Option<Self> {
        let goal = Self { turn_id, reply };
        let mut inner = state.lock().expect("supervisor state lock poisoned");
        let dispatch = if inner.closing
            || inner.snapshot.cancellation_requested
            || inner.snapshot.active_turn != Some(turn_id)
        {
            Err(AcpError::Transport(
                "Goal admission was cancelled before dispatch".into(),
            ))
        } else {
            transport.extension_observed(turn_id, &method, params)
        };
        match dispatch {
            Ok(()) => Some(goal),
            Err(error) => {
                goal.settle(&mut inner, Err(error));
                None
            }
        }
    }

    /// Settle a dispatch failure or the owner's matching completion event.
    /// For a provider response, earlier text is already forwarded to presentation.
    pub(super) fn settle(self, inner: &mut Inner, result: magistr_acp::Result<serde_json::Value>) {
        if inner.snapshot.active_turn == Some(self.turn_id) {
            match &result {
                Ok(_) => {
                    inner.snapshot.active_turn = None;
                    inner.goal_turn = None;
                    inner.completed_turns += 1;
                    if let Some(history) = &inner.history {
                        history.finished_turn();
                    }
                    inner.snapshot.cancellation_requested = false;
                    inner.snapshot.last_outcome = Some(TurnOutcome {
                        turn_id: self.turn_id,
                        stop_reason: Some(agent_client_protocol::schema::v1::StopReason::EndTurn),
                        error: None,
                    });
                    inner.emit_cockpit(AgentEvent::TurnEnded(TurnEnd::EndTurn));
                }
                Err(error) => {
                    let error = format!("Goal execution failed: {error}");
                    inner.finish(SessionResidency::Failed, &error);
                    inner.emit(Err(acp_error(error)));
                }
            }
        }
        let _ = self.reply.send(result);
    }
}
