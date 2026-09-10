//! The single resident transport task and its cancellation-safe resource guard.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use magistr_acp::{AcpEvent, AcpTransport};
use tokio::sync::mpsc;

use super::permissions::Permissions;
use super::{Command, Inner, SessionResidency, TurnOutcome, acp_error};
use crate::agent::message::AgentMessage;
use crate::error::Result;

use super::registration::Registration;

struct OwnerState(Arc<Mutex<Inner>>);

impl Drop for OwnerState {
    fn drop(&mut self) {
        let mut state = self.0.lock().expect("supervisor state lock poisoned");
        if state.snapshot.residency == SessionResidency::Resident {
            state.finish(
                SessionResidency::Terminated,
                "Session owner stopped before a provider terminal response",
            );
        }
        state.messages = None;
        state.snapshot.client_attached = false;
    }
}

pub(super) async fn run(
    mut transport: AcpTransport,
    mut events: mpsc::UnboundedReceiver<magistr_acp::Result<AcpEvent>>,
    mut permissions: Permissions,
    mut commands: mpsc::UnboundedReceiver<Command>,
    state: Arc<Mutex<Inner>>,
    _registration: Registration,
) -> Result<()> {
    let _owner = OwnerState(Arc::clone(&state));
    run_loop(
        &transport,
        &mut events,
        &mut permissions,
        &mut commands,
        &state,
    )
    .await;
    permissions.close();
    let result = transport
        .close()
        .await
        .map_err(|error| acp_error(format!("Session transport cleanup failed: {error}")));
    let mut inner = state.lock().expect("supervisor state lock poisoned");
    if let Err(error) = &result {
        inner.finish(SessionResidency::Failed, &error.to_string());
        inner.emit(Err(acp_error(error.to_string())));
    } else if inner.snapshot.residency == SessionResidency::Resident {
        inner.finish(
            SessionResidency::Terminated,
            "Session terminated before a provider terminal response",
        );
    }
    inner.messages = None;
    inner.snapshot.client_attached = false;
    result
}

type ControlFuture<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;

async fn run_loop(
    transport: &AcpTransport,
    events: &mut mpsc::UnboundedReceiver<magistr_acp::Result<AcpEvent>>,
    permissions: &mut Permissions,
    commands: &mut mpsc::UnboundedReceiver<Command>,
    state: &Arc<Mutex<Inner>>,
) {
    let mut session_id = None;
    let mut control: Option<ControlFuture<'_>> = None;
    loop {
        if !start_next(transport, permissions, state) {
            break;
        }
        tokio::select! {
            command = commands.recv() => match command {
                None | Some(Command::Terminate) => break,
                Some(Command::Wake) => {},
                Some(Command::Cancel { turn_id }) => {
                    let mut inner = state.lock().expect("supervisor state lock poisoned");
                    if turn_id.is_some() && turn_id == inner.snapshot.active_turn {
                        permissions.drain();
                        if let Err(error) = transport.cancel() {
                            fail(&mut inner, format!("Provider cancellation failed: {error}"));
                            break;
                        }
                    } else if inner.legacy_cancel && inner.snapshot.active_turn.is_none() {
                        break;
                    }
                }
                Some(Command::List { reply }) => {
                    control = Some(Box::pin(async move {
                        let result = transport.list_sessions(None).await
                            .map_err(|error| acp_error(format!("list_sessions failed: {error}")));
                        state.lock().expect("supervisor state lock poisoned").control_busy = false;
                        let _ = reply.send(result);
                    }));
                }
                Some(Command::Config { config_id, value, reply }) => {
                    let session_id = session_id.clone();
                    control = Some(Box::pin(async move {
                        let result = if let Some(session_id) = session_id {
                            transport.set_config_option(session_id, config_id, value).await
                                .map_err(|error| acp_error(format!("set_config_option failed: {error}")))
                        } else {
                            Err(acp_error("Session not initialized — cannot set config option"))
                        };
                        state.lock().expect("supervisor state lock poisoned").control_busy = false;
                        if let Some(reply) = reply {
                            let _ = reply.send(result);
                        } else {
                            let mut inner = state.lock().expect("supervisor state lock poisoned");
                            inner.emit(result.map(|options| AgentMessage::ConfigUpdate {
                                options: crate::agent::types::config_options_from_acp(&options),
                            }));
                        }
                    }));
                }
            },
            event = events.recv() => {
                let mut inner = state.lock().expect("supervisor state lock poisoned");
                match event {
                    Some(Ok(event)) => {
                        if let AcpEvent::Initialized { session_id: id, agent_capabilities, .. } = &event {
                            session_id = Some(id.clone());
                            inner.capabilities = Some((**agent_capabilities).clone());
                        }
                        if let AcpEvent::PromptResponse { response, .. } = &event {
                            if let Some(turn_id) = inner.snapshot.active_turn.take() {
                                inner.completed_turns += 1;
                                inner.snapshot.last_outcome = Some(TurnOutcome {
                                    turn_id, stop_reason: Some(response.stop_reason), error: None,
                                });
                                inner.snapshot.cancellation_requested = false;
                                // Cancellation, refusal and limits revoke speculative
                                // queued input; only success advances the FIFO.
                                if response.stop_reason != agent_client_protocol::schema::v1::StopReason::EndTurn {
                                    inner.discard_queue("provider ended the active turn without success");
                                }
                            } else {
                                fail(&mut inner, "Provider sent a terminal response without an active turn".into());
                                break;
                            }
                        }
                        let (messages, _, _) = crate::agent::convert_acp_event::convert_acp_event(&event);
                        for message in messages {
                            inner.emit(Ok(message));
                        }
                        if inner.legacy_cancel && inner.snapshot.active_turn.is_none() {
                            break;
                        }
                    }
                    Some(Err(error)) => {
                        fail(&mut inner, error.to_string());
                        break;
                    }
                    None => {
                        fail(&mut inner, "ACP event stream ended without session termination".into());
                        break;
                    }
                }
            },
            request = permissions.recv() => permissions.answer(request, state),
            () = async {
                match control.as_mut() {
                    Some(future) => future.await,
                    None => std::future::pending().await,
                }
            } => {
                control = None;
            }
        }
    }
}

fn start_next(
    transport: &AcpTransport,
    permissions: &mut Permissions,
    state: &Mutex<Inner>,
) -> bool {
    let mut inner = state.lock().expect("supervisor state lock poisoned");
    if inner.closing {
        return false;
    }
    if inner.snapshot.active_turn.is_some()
        || inner.snapshot.cancellation_requested
        || inner.control_busy
        || inner.legacy_cancel
    {
        return true;
    }
    if let Some((id, prompt)) = inner.queue.pop_front() {
        // Drain every old permission before activating the next turn. Replies
        // whose effect epoch was cancelled remain closed in the ACP bridge.
        permissions.drain();
        inner.snapshot.queued_turns = inner.queue.len();
        inner.snapshot.active_turn = Some(id);
        if let Err(error) = transport.prompt(&prompt) {
            fail(&mut inner, format!("Failed to send follow-up: {error}"));
            return false;
        }
    }
    true
}

fn fail(inner: &mut Inner, error: String) {
    inner.finish(SessionResidency::Failed, &error);
    inner.emit(Err(acp_error(error)));
}
