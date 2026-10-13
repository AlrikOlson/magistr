//! The single resident transport task and its cancellation-safe resource guard.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use magistr_acp::{AcpEvent, AcpTransport};
use magistr_application::sessions::SessionOptions;
use tokio::sync::mpsc;

use super::permissions::Permissions;
use super::{Command, Inner, SessionResidency, TurnOutcome, acp_error};
use crate::agent::message::AgentMessage;
use crate::agent::persistence::session_outcome::SessionOutcome;
use crate::error::Result;

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
        if let Some(history) = &state.history {
            history.settle_detached(outcome(&state));
        }
        state.messages = None;
        state.cockpit_events = None;
        state.snapshot.client_attached = false;
    }
}

pub(super) async fn run(
    mut transport: AcpTransport,
    mut events: mpsc::UnboundedReceiver<magistr_acp::Result<AcpEvent>>,
    mut permissions: Permissions,
    mut commands: mpsc::UnboundedReceiver<Command>,
    state: Arc<Mutex<Inner>>,
    options: Option<SessionOptions>,
) -> Result<()> {
    let _owner = OwnerState(Arc::clone(&state));
    run_loop(
        &transport,
        &mut events,
        &mut permissions,
        &mut commands,
        &state,
        options,
    )
    .await;
    permissions.close();
    let result = transport
        .close()
        .await
        .map_err(|error| acp_error(format!("Session transport cleanup failed: {error}")));
    let settled = {
        let inner = state.lock().expect("supervisor state lock poisoned");
        if result.is_err() {
            SessionOutcome::Failed
        } else {
            outcome(&inner)
        }
    };
    let history = state
        .lock()
        .expect("supervisor state lock poisoned")
        .history
        .clone();
    if let Some(history) = history
        && let Err(error) = history.settle(settled).await
    {
        crate::agent::persistence::warn_history(&error);
        let mut inner = state.lock().expect("supervisor state lock poisoned");
        inner.emit_cockpit(magistr_application::sessions::AgentEvent::Notice(
            "The session ended, but its history could not be saved. Check the folder used for saved sessions.".into(),
        ));
        inner.emit(Ok(AgentMessage::System {
            subtype: "history_save_failed".into(),
            data: serde_json::json!({"error": error.to_string()}),
        }));
    }
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
    inner.cockpit_events = None;
    inner.snapshot.client_attached = false;
    result
}

fn outcome(inner: &Inner) -> SessionOutcome {
    use agent_client_protocol::schema::v1::StopReason;
    if inner.snapshot.residency == SessionResidency::Failed {
        SessionOutcome::Failed
    } else if inner.snapshot.active_turn.is_some() {
        SessionOutcome::Stopped
    } else {
        match inner.snapshot.last_outcome.as_ref() {
            Some(TurnOutcome {
                stop_reason: Some(StopReason::EndTurn),
                ..
            }) => SessionOutcome::Completed,
            Some(TurnOutcome {
                stop_reason: Some(StopReason::Cancelled),
                ..
            }) => SessionOutcome::Cancelled,
            Some(TurnOutcome { error: Some(_), .. })
                if inner.snapshot.residency != SessionResidency::Terminated =>
            {
                SessionOutcome::Failed
            }
            Some(TurnOutcome {
                stop_reason: Some(_),
                ..
            }) => SessionOutcome::Failed,
            _ => SessionOutcome::Stopped,
        }
    }
}

type ControlFuture<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;

async fn run_loop(
    transport: &AcpTransport,
    events: &mut mpsc::UnboundedReceiver<magistr_acp::Result<AcpEvent>>,
    permissions: &mut Permissions,
    commands: &mut mpsc::UnboundedReceiver<Command>,
    state: &Arc<Mutex<Inner>>,
    mut options: Option<SessionOptions>,
) {
    let mut session_id = None;
    let mut control: Option<ControlFuture<'_>> = None;
    let mut goal: Option<super::goal::Goal> = None;
    // The process-wide interrupt is observed once; a caller's own
    // `interrupt` takes the same path through the command lane.
    let shutdown = crate::utils::shutdown::Shutdown::token();
    let mut interrupted = false;
    loop {
        if !start_next(transport, permissions, state) {
            break;
        }
        tokio::select! {
            () = shutdown.cancelled(), if !interrupted => {
                interrupted = true;
                if !drain(transport, permissions, state) {
                    break;
                }
            }
            command = commands.recv() => match command {
                None | Some(Command::Terminate) => break,
                Some(Command::Wake) => {},
                Some(Command::Interrupt) => {
                    interrupted = true;
                    if !drain(transport, permissions, state) {
                        break;
                    }
                }
                Some(Command::Detach) => permissions.drain(),
                Some(Command::Permission { id, option_id }) => permissions.reply(id, option_id, state),
                Some(Command::Constraint { id, condition }) => permissions.constrain(id, &condition, state),
                Some(Command::Delegate(key)) => permissions.delegate(&key, state),
                Some(Command::KillTerminal(id)) => {
                    if let Err(error) = transport.kill_terminal(&id) {
                        state.lock().expect("supervisor state lock poisoned").emit(Err(acp_error(error.to_string())));
                    }
                }
                Some(Command::BeginGoal { method, params, reply, turn_id }) => {
                    goal = super::goal::Goal::begin(transport, state, method, params, turn_id, reply);
                }
                Some(Command::ClearGoal { method, params }) => {
                    if control.is_some() {
                        state.lock().expect("supervisor state lock poisoned").emit(Err(acp_error("A session control request is already active")));
                    } else {
                        state.lock().expect("supervisor state lock poisoned").control_busy = true;
                        control = Some(Box::pin(async move {
                            let result = match transport.extension(&method, params) {
                                Ok(response) => response.await.unwrap_or_else(|_| Err(magistr_acp::AcpError::Transport("Goal clear response channel closed".into()))),
                                Err(error) => Err(error),
                            };
                            let mut inner = state.lock().expect("supervisor state lock poisoned");
                            inner.control_busy = false;
                            if let Err(error) = result {
                                fail(&mut inner, format!("Failed to clear the delegated goal: {error}"));
                            }
                        }));
                    }
                }
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
                        if let Some(history) = &inner.history { history.observe(&event); }
                        if let AcpEvent::ExtensionResponse { request_id, result } = event {
                            permissions.drain();
                            match goal.take() {
                                Some(pending) if pending.turn_id == request_id => {
                                    pending.settle(&mut inner, result.map_err(magistr_acp::AcpError::transport));
                                }
                                _ => {
                                    fail(&mut inner, "Provider extension response did not match the active goal".into());
                                    break;
                                }
                            }
                            continue;
                        }
                        if let AcpEvent::Initialized { session_id: id, agent_capabilities, .. } = &event {
                            session_id = Some(id.clone());
                            if options.is_none() && inner.snapshot.active_turn.is_some()
                                && let Some(history) = &inner.history { history.begin_turn(); }
                            inner.capabilities = Some((**agent_capabilities).clone());
                            if let Some(options) = options.take() {
                                control = Some(Box::pin(super::opening::initialize(transport, event, options, state)));
                                continue;
                            }
                        }
                        if let AcpEvent::System { subtype, data } = &event
                            && subtype == "permission_lapsed"
                        {
                            permissions.lapse(data);
                            continue;
                        }
                        if let AcpEvent::System { subtype, data } = &event
                            && subtype == "prompt_rejected"
                        {
                            let error = data.get("error").and_then(serde_json::Value::as_str)
                                .unwrap_or("Your message could not be sent.").to_owned();
                            if let Some(turn_id) = inner.snapshot.active_turn.take() {
                                inner.snapshot.last_outcome = Some(TurnOutcome {
                                    turn_id, stop_reason: None, error: Some(error.clone()),
                                });
                            }
                            inner.snapshot.cancellation_requested = false;
                            inner.discard_queue(&error);
                            inner.emit(Ok(AgentMessage::Result {
                                subtype: "message_not_sent".into(),
                                duration_ms: 0,
                                duration_api_ms: 0,
                                is_error: true,
                                num_turns: 0,
                                session_id: crate::agent::message::SessionId::new(session_id.as_deref().unwrap_or_default()),
                                total_cost_usd: None,
                                usage: None,
                                result: Some(error),
                            }));
                        }
                        if let AcpEvent::PromptResponse { response, .. } = &event {
                            permissions.drain();
                            if inner.goal_turn.is_some() {
                                fail(&mut inner, "Provider sent a prompt response for a goal extension".into());
                                break;
                            }
                            if let Some(turn_id) = inner.snapshot.active_turn.take() {
                                inner.completed_turns += 1;
                                if let Some(history) = &inner.history { history.finished_turn(); }
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
                        inner.emit_cockpit(crate::agent::translate::translate(&Ok(event)));
                        for message in messages {
                            inner.emit(Ok(message));
                        }
                        if inner.legacy_cancel && inner.snapshot.active_turn.is_none() {
                            break;
                        }
                    }
                    Some(Err(error)) => {
                        permissions.drain();
                        fail(&mut inner, error.to_string());
                        break;
                    }
                    None => {
                        permissions.drain();
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

/// Wind down after an interrupt: no further admission, queued input
/// discarded, the active turn asked to stop. `false` when nothing is
/// active and the owner can close at once; otherwise the provider's
/// terminal response ends the loop through [`start_next`].
fn drain(transport: &AcpTransport, permissions: &mut Permissions, state: &Mutex<Inner>) -> bool {
    let mut inner = state.lock().expect("supervisor state lock poisoned");
    inner.draining = true;
    inner.discard_queue("interrupted");
    if inner.snapshot.active_turn.is_none() {
        return false;
    }
    if inner.snapshot.cancellation_requested {
        return true;
    }
    inner.snapshot.cancellation_requested = true;
    permissions.drain();
    if let Err(error) = transport.cancel() {
        fail(&mut inner, format!("Provider cancellation failed: {error}"));
        return false;
    }
    true
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
    if inner.draining && inner.snapshot.active_turn.is_none() {
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
        if let Some(history) = &inner.history {
            history.begin_turn();
        }
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
