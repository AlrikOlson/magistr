//! Goal extensions settle through the same turn and permission lifecycle.

use std::sync::{Arc, Mutex};

use magistr_acp::{AcpError, AcpTransport};
use magistr_cockpit::agent::{AgentEvent, TurnEnd};
use tokio::sync::oneshot;

use super::{Inner, SessionResidency, TurnOutcome, acp_error};

pub(super) async fn run(
    transport: &AcpTransport,
    state: &Arc<Mutex<Inner>>,
    method: String,
    params: serde_json::Value,
    turn_id: u64,
    reply: oneshot::Sender<magistr_acp::Result<serde_json::Value>>,
) {
    let response = {
        let inner = state.lock().expect("supervisor state lock poisoned");
        if inner.closing
            || inner.snapshot.cancellation_requested
            || inner.snapshot.active_turn != Some(turn_id)
        {
            Err(AcpError::Transport(
                "Goal admission was cancelled before dispatch".into(),
            ))
        } else {
            transport.extension(&method, params)
        }
    };
    let result = match response {
        Ok(response) => response
            .await
            .unwrap_or_else(|_| Err(AcpError::Transport("Goal response channel closed".into()))),
        Err(error) => Err(error),
    };
    let mut inner = state.lock().expect("supervisor state lock poisoned");
    if inner.snapshot.active_turn == Some(turn_id) {
        match &result {
            Ok(_) => {
                inner.snapshot.active_turn = None;
                inner.goal_turn = None;
                inner.completed_turns += 1;
                inner.snapshot.cancellation_requested = false;
                inner.snapshot.last_outcome = Some(TurnOutcome {
                    turn_id,
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
    let _ = reply.send(result);
}
