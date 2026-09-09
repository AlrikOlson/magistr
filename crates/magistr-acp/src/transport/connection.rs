//! Assemble the client connection: handlers for the agent's requests, then
//! the foreground command loop.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use agent_client_protocol::schema::v1::{
    CreateTerminalRequest, KillTerminalRequest, McpServer, Meta, ReadTextFileRequest,
    ReleaseTerminalRequest, RequestPermissionRequest, SessionNotification, TerminalOutputRequest,
    WaitForTerminalExitRequest, WriteTextFileRequest,
};
use agent_client_protocol::{Client, ConnectTo, on_receive_notification, on_receive_request};
use tokio::process::Child;
use tokio::sync::mpsc;

use super::command::AcpCommand;
use super::command_loop::{self, HandshakeSender};
use crate::handlers::terminal::{self, TerminalManager};
use crate::handlers::{fs, permission};
use crate::stderr_tail::StderrTail;
use crate::{AcpError, AcpEvent, AcpPermissionRequest, AcpSessionMode, Result};

/// Everything the connection task owns.
pub(crate) struct ConnectionInputs {
    pub child: Option<Child>,
    pub cwd: PathBuf,
    pub session_mode: AcpSessionMode,
    pub mcp_servers: Vec<McpServer>,
    pub session_meta: Option<Meta>,
    pub cmd_rx: mpsc::UnboundedReceiver<AcpCommand>,
    pub event_tx: mpsc::UnboundedSender<Result<AcpEvent>>,
    pub perm_tx: mpsc::UnboundedSender<AcpPermissionRequest>,
    pub ready: Arc<AtomicBool>,
    pub handshake_tx: HandshakeSender,
    pub stderr_tail: StderrTail,
    /// The agent's terminals, shared with the transport handle so the
    /// cockpit's held kill reaches the child `terminal/create` spawned.
    pub terminals: TerminalManager,
}

/// Build the `Client` connection over `component` and run it to completion.
///
/// Handlers that wait on something outside the connection — the application's
/// permission decision, a terminal's exit, disk I/O — are spawned with
/// `cx.spawn` and respond from the task, because a handler holds the dispatch
/// loop while it runs. The rest respond inline.
pub(crate) async fn run_connection(
    component: impl ConnectTo<Client> + 'static,
    inputs: ConnectionInputs,
) {
    let event_tx = inputs.event_tx.clone();
    let perm_tx = inputs.perm_tx.clone();
    let terminals: TerminalManager = Arc::clone(&inputs.terminals);

    let outcome = Client
        .builder()
        .name("magistr")
        .on_receive_notification(
            {
                let event_tx = event_tx.clone();
                async move |n: SessionNotification, _cx| {
                    let _ = event_tx.send(Ok(AcpEvent::SessionUpdate {
                        update: Box::new(n.update),
                        session_id: n.session_id.to_string(),
                    }));
                    Ok(())
                }
            },
            on_receive_notification!(),
        )
        .on_receive_request(
            {
                let (perm_tx, event_tx) = (perm_tx.clone(), event_tx.clone());
                async move |req: RequestPermissionRequest, responder, cx| {
                    let (perm_tx, event_tx) = (perm_tx.clone(), event_tx.clone());
                    cx.spawn(async move {
                        responder
                            .respond(permission::request_permission(req, &perm_tx, &event_tx).await)
                    })?;
                    Ok(())
                }
            },
            on_receive_request!(),
        )
        .on_receive_request(
            {
                let event_tx = event_tx.clone();
                async move |req: ReadTextFileRequest, responder, cx| {
                    let event_tx = event_tx.clone();
                    cx.spawn(async move {
                        responder.respond_with_result(fs::read_text_file(req, &event_tx).await)
                    })?;
                    Ok(())
                }
            },
            on_receive_request!(),
        )
        .on_receive_request(
            {
                let event_tx = event_tx.clone();
                async move |req: WriteTextFileRequest, responder, cx| {
                    let event_tx = event_tx.clone();
                    cx.spawn(async move {
                        responder.respond_with_result(fs::write_text_file(req, &event_tx).await)
                    })?;
                    Ok(())
                }
            },
            on_receive_request!(),
        )
        .on_receive_request(
            {
                let (terminals, event_tx) = (Arc::clone(&terminals), event_tx.clone());
                async move |req: CreateTerminalRequest, responder, _cx| {
                    responder
                        .respond_with_result(terminal::create_terminal(req, &terminals, &event_tx))
                }
            },
            on_receive_request!(),
        )
        .on_receive_request(
            {
                let terminals = Arc::clone(&terminals);
                async move |req: TerminalOutputRequest, responder, _cx| {
                    responder.respond_with_result(terminal::terminal_output(&req, &terminals))
                }
            },
            on_receive_request!(),
        )
        .on_receive_request(
            {
                let terminals = Arc::clone(&terminals);
                async move |req: WaitForTerminalExitRequest, responder, cx| {
                    let terminals = Arc::clone(&terminals);
                    cx.spawn(async move {
                        responder.respond_with_result(
                            terminal::wait_for_terminal_exit(&req, &terminals).await,
                        )
                    })?;
                    Ok(())
                }
            },
            on_receive_request!(),
        )
        .on_receive_request(
            {
                let terminals = Arc::clone(&terminals);
                async move |req: KillTerminalRequest, responder, _cx| {
                    responder.respond_with_result(terminal::kill_terminal(&req, &terminals))
                }
            },
            on_receive_request!(),
        )
        .on_receive_request(
            {
                let (terminals, event_tx) = (Arc::clone(&terminals), event_tx.clone());
                async move |req: ReleaseTerminalRequest, responder, _cx| {
                    responder.respond_with_result(terminal::release_terminal(
                        &req, &terminals, &event_tx,
                    ))
                }
            },
            on_receive_request!(),
        )
        .connect_with(component, async move |cx| {
            command_loop::run(cx, inputs).await;
            Ok(())
        })
        .await;

    if let Err(e) = outcome {
        tracing::warn!(error = %e, "ACP connection ended with an error");
        let _ = event_tx.send(Err(AcpError::transport(format!(
            "ACP connection ended: {e}"
        ))));
    }
}
