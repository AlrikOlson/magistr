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
use magistr_process::ReapingChild as Child;
use tokio::sync::mpsc;

use super::command::AcpCommand;
use super::command_loop::{self, HandshakeSender};
use crate::handlers::effects::EffectBridge;
use crate::stderr_tail::StderrTail;
use crate::{AcpError, AcpEvent, AcpSessionMode, Result};

/// Everything the connection task owns.
pub(crate) struct ConnectionInputs {
    pub child: Option<Child>,
    pub cwd: PathBuf,
    pub session_mode: AcpSessionMode,
    pub mcp_servers: Vec<McpServer>,
    pub session_meta: Option<Meta>,
    pub cmd_rx: mpsc::UnboundedReceiver<AcpCommand>,
    pub event_tx: mpsc::UnboundedSender<Result<AcpEvent>>,
    pub ready: Arc<AtomicBool>,
    pub handshake_tx: HandshakeSender,
    pub stderr_tail: StderrTail,
    pub effects: Arc<EffectBridge>,
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
    let effects = Arc::clone(&inputs.effects);

    // Every host effect is asynchronous, including admission before a spawn.
    // Keeping all of them off the dispatch loop lets cancel and updates arrive
    // while the application is deciding a permission request.
    macro_rules! effect_handler {
        ($builder:expr, $request:ty, $method:ident) => {
            $builder.on_receive_request(
                {
                    let effects = Arc::clone(&effects);
                    async move |req: $request, responder, cx| {
                        let effects = effects.for_request();
                        cx.spawn(async move {
                            responder.respond_with_result(effects.$method(req).await)
                        })?;
                        Ok(())
                    }
                },
                on_receive_request!(),
            )
        };
    }
    let client = Client
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
                let effects = Arc::clone(&effects);
                async move |req: RequestPermissionRequest, responder, cx| {
                    let effects = effects.for_request();
                    cx.spawn(
                        async move { responder.respond(effects.request_permission(req).await) },
                    )?;
                    Ok(())
                }
            },
            on_receive_request!(),
        );
    let client = effect_handler!(client, ReadTextFileRequest, read_text_file);
    let client = effect_handler!(client, WriteTextFileRequest, write_text_file);
    let client = effect_handler!(client, CreateTerminalRequest, create_terminal);
    let client = effect_handler!(client, TerminalOutputRequest, terminal_output);
    let client = effect_handler!(client, WaitForTerminalExitRequest, wait_for_terminal_exit);
    let client = effect_handler!(client, KillTerminalRequest, kill_terminal_request);
    let client = effect_handler!(client, ReleaseTerminalRequest, release_terminal);
    let outcome = client
        .connect_with(component, async move |cx| {
            command_loop::run(cx, inputs).await;
            Ok(())
        })
        .await;

    effects.close();
    if let Err(e) = outcome {
        tracing::warn!(error = %e, "ACP connection ended with an error");
        let _ = event_tx.send(Err(AcpError::transport(format!(
            "ACP connection ended: {e}"
        ))));
    }
}
