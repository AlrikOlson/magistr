//! The foreground of the connection: handshake, then serve commands.
//!
//! Runs inside `connect_with`'s closure — outside the dispatch loop — so it
//! may `block_task()` on responses while handlers keep being served.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use agent_client_protocol::schema::v1::{
    CancelNotification, ListSessionsRequest, PromptRequest, SessionConfigId, SessionId,
    SetSessionConfigOptionRequest,
};
use agent_client_protocol::{Agent, ConnectionTo};
use magistr_process::ReapingChild as Child;
use tokio::sync::{mpsc, oneshot};

use super::command::AcpCommand;
use super::connection::ConnectionInputs;
use super::handshake::{AcpHandshake, HandshakeInputs, acp_handshake};
use super::prompt_content::PromptContent;
use super::wait_child::wait_child;
use crate::handlers::effects::EffectBridge;
use crate::stderr_tail::StderrTail;
use crate::{AcpError, AcpEvent, Result};

/// How long `Close` waits for an in-flight prompt to acknowledge the cancel.
const CLOSE_GRACE: Duration = Duration::from_secs(3);

/// Why the loop stopped.
enum Stop {
    /// A `Close` command, or the command channel closed: shut down quietly.
    Requested,
    /// The agent went away — already reported on the event channel.
    AgentGone,
}

/// Run the handshake and then serve commands until the transport closes or
/// the agent goes away. Errors before the handshake completes are routed to
/// `handshake_tx` (the caller's `connect()` is waiting on it); later ones to
/// the event channel.
pub(crate) async fn run(cx: ConnectionTo<Agent>, inputs: ConnectionInputs) {
    let ConnectionInputs {
        mut child,
        cwd,
        session_mode,
        mcp_servers,
        session_meta,
        mut cmd_rx,
        event_tx,
        ready,
        handshake_tx,
        stderr_tail,
        effects,
        ..
    } = inputs;

    let handshake = acp_handshake(
        &cx,
        &mut child,
        HandshakeInputs {
            client_capabilities: effects.client_capabilities(),
            cwd: &cwd,
            session_mode: &session_mode,
            mcp_servers: &mcp_servers,
            session_meta: &session_meta,
            stderr_tail: &stderr_tail,
        },
    )
    .await;
    let handshake = match handshake {
        Ok(handshake) => handshake,
        Err(e) => {
            let _ = handshake_tx.send(Err(e));
            kill(&mut child).await;
            return;
        }
    };

    let AcpHandshake {
        session_id,
        agent_capabilities,
        config_options,
        agent_meta,
    } = handshake.clone();
    let images = agent_capabilities.prompt_capabilities.image;
    effects.bind_session(session_id.to_string());
    let _ = event_tx.send(Ok(AcpEvent::Initialized {
        agent_capabilities: Box::new(agent_capabilities),
        session_id: session_id.to_string(),
        config_options,
        agent_meta,
    }));
    ready.store(true, Ordering::SeqCst);
    let _ = handshake_tx.send(Ok(handshake));

    let mut session = Session {
        images,
        cwd,
        cx,
        session_id,
        child,
        event_tx,
        stderr_tail,
        effects,
    };
    loop {
        let command = tokio::select! {
            biased;
            cmd = cmd_rx.recv() => match cmd {
                Some(cmd) => cmd,
                None => break,
            },
            () = session.cx.incoming_closed() => {
                session.report_closed();
                break;
            }
            code = wait_child(&mut session.child) => {
                session.report_exit(code);
                break;
            }
        };
        match session.serve(command, &mut cmd_rx).await {
            Ok(()) => {}
            Err(Stop::Requested | Stop::AgentGone) => break,
        }
    }
    ready.store(false, Ordering::SeqCst);
    session.effects.close();
    kill(&mut session.child).await;
}

/// Everything the loop needs once the session exists.
struct Session {
    images: bool,
    cwd: PathBuf,
    cx: ConnectionTo<Agent>,
    session_id: SessionId,
    child: Option<Child>,
    event_tx: mpsc::UnboundedSender<Result<AcpEvent>>,
    stderr_tail: StderrTail,
    effects: Arc<EffectBridge>,
}

impl Session {
    async fn serve(
        &mut self,
        command: AcpCommand,
        cmd_rx: &mut mpsc::UnboundedReceiver<AcpCommand>,
    ) -> std::result::Result<(), Stop> {
        match command {
            AcpCommand::Prompt(text) => self.prompt(text, cmd_rx).await,
            // A cancel with no turn in flight has nothing to cancel.
            AcpCommand::Cancel => Ok(()),
            AcpCommand::ListSessions { cwd, response_tx } => {
                let result = self
                    .cx
                    .send_request(ListSessionsRequest::new().cwd(cwd))
                    .block_task()
                    .await
                    .map(|r| r.sessions)
                    .map_err(|e| AcpError::transport(format!("ACP list_sessions failed: {e}")));
                let _ = response_tx.send(result);
                Ok(())
            }
            AcpCommand::SetConfigOption {
                session_id,
                config_id,
                value,
                response_tx,
            } => {
                let request = SetSessionConfigOptionRequest::new(
                    SessionId::new(session_id.as_str()),
                    SessionConfigId::new(config_id.as_str()),
                    value,
                );
                let result = self
                    .cx
                    .send_request(request)
                    .block_task()
                    .await
                    .map(|r| r.config_options)
                    .map_err(|e| AcpError::transport(format!("ACP set_config_option failed: {e}")));
                let _ = response_tx.send(result);
                Ok(())
            }
            AcpCommand::Extension {
                method,
                params,
                reply,
            } => {
                reply.request(&self.cx, method, params, self.event_tx.clone());
                Ok(())
            }
            AcpCommand::Close => Err(Stop::Requested),
        }
    }

    /// One turn: send the prompt, then keep serving cancel/close and watching
    /// the agent until the `session/prompt` response arrives.
    async fn prompt(
        &mut self,
        text: String,
        cmd_rx: &mut mpsc::UnboundedReceiver<AcpCommand>,
    ) -> std::result::Result<(), Stop> {
        let content = match PromptContent::resolve(&text, &self.cwd, self.images).await {
            Ok(content) => content,
            Err(message) => {
                let _ = self.event_tx.send(Ok(AcpEvent::System {
                    subtype: "prompt_rejected".into(),
                    data: serde_json::json!({"text":text,"error":message}),
                }));
                return Ok(());
            }
        };
        self.effects.begin_turn();
        let request = PromptRequest::new(self.session_id.clone(), content);
        let response = self.cx.send_request(request).block_task();
        tokio::pin!(response);

        let outcome = loop {
            tokio::select! {
                biased;
                cmd = cmd_rx.recv() => match cmd {
                    Some(AcpCommand::Cancel) => self.send_cancel(),
                    Some(AcpCommand::Close) | None => {
                        self.send_cancel();
                        let _ = tokio::time::timeout(CLOSE_GRACE, &mut response).await;
                        return Err(Stop::Requested);
                    }
                    // A second prompt while one is in flight is a caller bug; ignore it.
                    Some(AcpCommand::Prompt(_)) => {}
                    Some(AcpCommand::ListSessions { response_tx, .. }) => {
                        let _ = response_tx.send(Err(AcpError::transport(
                            "Cannot list sessions while a prompt is in-flight",
                        )));
                    }
                    Some(AcpCommand::SetConfigOption { response_tx, .. }) => {
                        let _ = response_tx.send(Err(AcpError::transport(
                            "Cannot set config option while a prompt is in-flight",
                        )));
                    }
                    // An extension call mid-turn is exactly what a
                    // steering-style extension is for.
                    Some(AcpCommand::Extension { method, params, reply }) => {
                        reply.request(&self.cx, method, params, self.event_tx.clone());
                    }
                },
                result = &mut response => break result,
                () = self.cx.incoming_closed() => {
                    self.report_closed();
                    return Err(Stop::AgentGone);
                }
                code = wait_child(&mut self.child) => {
                    self.report_exit(code);
                    return Err(Stop::AgentGone);
                }
            }
        };

        match outcome {
            Ok(prompt_response) => {
                let _ = self.event_tx.send(Ok(AcpEvent::PromptResponse {
                    response: prompt_response,
                    session_id: self.session_id.to_string(),
                }));
            }
            Err(e) => {
                let _ = self
                    .event_tx
                    .send(Err(AcpError::transport(format!("ACP prompt failed: {e}"))));
            }
        }
        Ok(())
    }

    fn send_cancel(&self) {
        self.effects.cancel();
        let _ = self
            .cx
            .send_notification(CancelNotification::new(self.session_id.clone()));
    }

    fn report_closed(&self) {
        let _ = self.event_tx.send(Err(self
            .stderr_tail
            .attach(AcpError::transport("ACP agent closed the connection"))));
    }

    fn report_exit(&self, code: i32) {
        tracing::error!(exit_code = code, "ACP subprocess exited while connected");
        let _ = self
            .event_tx
            .send(Err(self.stderr_tail.attach(AcpError::transport(format!(
                "ACP subprocess exited unexpectedly (exit code: {code})"
            )))));
    }
}

async fn kill(child: &mut Option<Child>) {
    if let Some(child) = child {
        let _ = child.kill().await;
    }
}

/// Keep the oneshot type local to this module's callers.
pub(crate) type HandshakeSender = oneshot::Sender<Result<AcpHandshake>>;
