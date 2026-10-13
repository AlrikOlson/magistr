//! `initialize`, then `session/new` or `session/load`, watching the child.

use std::path::Path;
use std::time::Duration;

use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::schema::v1::{
    AgentCapabilities, ClientCapabilities, Implementation, InitializeRequest, LoadSessionRequest,
    McpServer, Meta, NewSessionRequest, SessionConfigOption, SessionId,
};
use agent_client_protocol::{Agent, ConnectionTo};
use magistr_process::ReapingChild as Child;

use super::wait_child::{exit_code, wait_child};
use crate::stderr_tail::StderrTail;
use crate::{AcpError, AcpSessionMode, Result};

/// Grace period after a failed handshake for the agent to be reaped and its
/// stderr drained, so the error can quote the exit status and last lines.
const REAP_GRACE: Duration = Duration::from_millis(200);

/// What the handshake yields once the agent has accepted a session.
#[derive(Clone)]
pub(crate) struct AcpHandshake {
    pub session_id: SessionId,
    pub agent_capabilities: AgentCapabilities,
    pub config_options: Option<Vec<SessionConfigOption>>,
    pub agent_meta: Option<Meta>,
}

pub(crate) struct HandshakeInputs<'a> {
    pub client_capabilities: ClientCapabilities,
    pub cwd: &'a Path,
    pub session_mode: &'a AcpSessionMode,
    pub mcp_servers: &'a [McpServer],
    pub session_meta: &'a Option<Meta>,
    pub stderr_tail: &'a StderrTail,
}

/// Run the handshake while watching the child. An agent that exits with a
/// request in flight would otherwise leave that request pending forever —
/// which is how a one-line refusal used to surface as a 30 s timeout. Errors
/// name the phase they failed in and, when the agent died, its exit status.
pub(crate) async fn acp_handshake(
    cx: &ConnectionTo<Agent>,
    child: &mut Option<Child>,
    inputs: HandshakeInputs<'_>,
) -> Result<AcpHandshake> {
    // Which request was in flight when the agent died — the future must stay
    // `Send`, so an atomic index into PHASES rather than a Cell.
    const PHASES: [&str; 4] = [
        "initialize",
        "session/load",
        "session/new",
        "session/configure",
    ];
    let phase = std::sync::atomic::AtomicUsize::new(0);
    let set_phase = |name: &str| {
        if let Some(i) = PHASES.iter().position(|p| *p == name) {
            phase.store(i, std::sync::atomic::Ordering::Relaxed);
        }
    };
    let requests = async {
        let init = InitializeRequest::new(ProtocolVersion::V1)
            .client_capabilities(inputs.client_capabilities.clone())
            .client_info(Implementation::new("magistr", env!("CARGO_PKG_VERSION")));
        let init_response = cx
            .send_request(init)
            .block_task()
            .await
            .map_err(|e| AcpError::connection(format!("ACP initialize failed: {e}")))?;
        let agent_capabilities = init_response.agent_capabilities;
        let agent_meta = init_response.meta;

        let (session_id, config_options) = match inputs.session_mode {
            AcpSessionMode::Load { session_id } if agent_capabilities.load_session => {
                set_phase("session/load");
                let sid = SessionId::new(session_id.as_str());
                let response = cx
                    .send_request(
                        LoadSessionRequest::new(sid.clone(), inputs.cwd)
                            .mcp_servers(inputs.mcp_servers.to_vec())
                            .meta(inputs.session_meta.clone()),
                    )
                    .block_task()
                    .await
                    .map_err(|e| AcpError::connection(format!("ACP session/load failed: {e}")))?;
                // session/load does not echo the id — it is the one we asked for.
                (sid, response.config_options)
            }
            AcpSessionMode::Load { session_id } => {
                return Err(AcpError::connection(format!(
                    "Agent does not support session resume; requested session '{session_id}' cannot be loaded"
                )));
            }
            AcpSessionMode::New => {
                set_phase("session/new");
                new_session(cx, inputs.cwd, inputs.mcp_servers, inputs.session_meta).await?
            }
        };
        set_phase("session/configure");
        let config_options = super::session_choices::configure(
            cx,
            &session_id,
            config_options,
            inputs.session_meta.as_ref(),
            agent_meta.as_ref(),
        )
        .await?;
        Ok(AcpHandshake {
            session_id,
            agent_capabilities,
            config_options,
            agent_meta,
        })
    };
    tokio::pin!(requests);

    let outcome = tokio::select! {
        result = &mut requests => result,
        code = wait_child(child) => Err(AcpError::connection(format!(
            "ACP agent exited during {} (exit code: {code})",
            PHASES[phase.load(std::sync::atomic::Ordering::Relaxed)],
        ))),
    };

    match outcome {
        Ok(handshake) => Ok(handshake),
        Err(e) => Err(annotate_failure(e, child, inputs.stderr_tail).await),
    }
}

async fn new_session(
    cx: &ConnectionTo<Agent>,
    cwd: &Path,
    mcp_servers: &[McpServer],
    session_meta: &Option<Meta>,
) -> Result<(SessionId, Option<Vec<SessionConfigOption>>)> {
    let response = cx
        .send_request(
            NewSessionRequest::new(cwd)
                .mcp_servers(mcp_servers.to_vec())
                .meta(session_meta.clone()),
        )
        .block_task()
        .await
        .map_err(|e| AcpError::connection(format!("ACP session/new failed: {e}")))?;
    Ok((response.session_id, response.config_options))
}

/// Give a dying agent a moment to be reaped so the report carries its exit
/// status, then attach its last stderr lines.
async fn annotate_failure(
    error: AcpError,
    child: &mut Option<Child>,
    stderr_tail: &StderrTail,
) -> AcpError {
    let exited = match child {
        Some(child) => tokio::time::timeout(REAP_GRACE, child.wait())
            .await
            .ok()
            .map(exit_code),
        None => None,
    };
    if exited.is_some() {
        // Let the stderr drain task catch up with the last lines.
        tokio::time::sleep(REAP_GRACE).await;
    }
    let error = match (error, exited) {
        (AcpError::Connection(m), Some(code)) if !m.contains("exit code") => {
            AcpError::Connection(format!("{m} (agent exit code: {code})"))
        }
        (e, _) => e,
    };
    stderr_tail.attach(error)
}
