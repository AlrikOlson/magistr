//! Translate ACP requests into the host's configured application capability.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use agent_client_protocol::Error;
use agent_client_protocol::schema::v1::{
    ClientCapabilities, CreateTerminalRequest, CreateTerminalResponse, FileSystemCapabilities,
    KillTerminalRequest, KillTerminalResponse, PermissionOption, PermissionOptionKind,
    ReadTextFileRequest, ReadTextFileResponse, ReleaseTerminalRequest, ReleaseTerminalResponse,
    RequestPermissionOutcome, RequestPermissionRequest, RequestPermissionResponse,
    TerminalExitStatus, TerminalOutputRequest, TerminalOutputResponse, ToolCallUpdate,
    ToolCallUpdateFields, ToolKind, WaitForTerminalExitRequest, WaitForTerminalExitResponse,
    WriteTextFileRequest, WriteTextFileResponse,
};
use magistr_application::effects::{
    EffectAuthorizer, EffectError, EffectFuture, EffectPort, EffectRequest, EffectResponse,
    TerminalExit,
};
use tokio::sync::{mpsc, watch};

use super::permission;
use crate::{AcpEvent, AcpPermissionRequest, Result};

const HOST_EFFECT_PREFIX: &str = "__magistr_host_effect:";

#[derive(Clone, Default)]
struct TurnState {
    session_id: Option<String>,
    generation: u64,
    cancelled: bool,
}

/// One connection's capability routing and revocable turn authorization.
pub(crate) struct EffectBridge {
    port: Option<Arc<dyn EffectPort>>,
    perm_tx: mpsc::UnboundedSender<AcpPermissionRequest>,
    event_tx: mpsc::UnboundedSender<Result<AcpEvent>>,
    state: watch::Sender<TurnState>,
    request_generation: Option<u64>,
}

impl EffectBridge {
    pub(crate) fn new(
        port: Option<Arc<dyn EffectPort>>,
        perm_tx: mpsc::UnboundedSender<AcpPermissionRequest>,
        event_tx: mpsc::UnboundedSender<Result<AcpEvent>>,
    ) -> Self {
        let (state, _) = watch::channel(TurnState::default());
        Self {
            port,
            perm_tx,
            event_tx,
            state,
            request_generation: None,
        }
    }

    /// Capture the turn when the wire request arrives, before its task is scheduled.
    pub(crate) fn for_request(&self) -> Self {
        Self {
            port: self.port.clone(),
            perm_tx: self.perm_tx.clone(),
            event_tx: self.event_tx.clone(),
            state: self.state.clone(),
            request_generation: Some(self.state.borrow().generation),
        }
    }

    pub(crate) fn client_capabilities(&self) -> ClientCapabilities {
        let capabilities = self
            .port
            .as_ref()
            .map(|port| port.capabilities())
            .unwrap_or_default();
        ClientCapabilities::new()
            .fs(FileSystemCapabilities::new()
                .read_text_file(capabilities.read_text_file)
                .write_text_file(capabilities.write_text_file))
            .terminal(capabilities.terminal)
    }

    pub(crate) fn bind_session(&self, session_id: String) {
        self.state
            .send_modify(|state| state.session_id = Some(session_id));
    }

    pub(crate) fn begin_turn(&self) {
        self.state.send_modify(|state| {
            state.generation = state.generation.wrapping_add(1);
            state.cancelled = false;
        });
    }

    pub(crate) fn cancel(&self) {
        self.state.send_modify(|state| {
            state.generation = state.generation.wrapping_add(1);
            state.cancelled = true;
        });
    }

    pub(crate) fn close(&self) {
        self.cancel();
        if let Some(port) = &self.port {
            port.close();
        }
    }

    pub(crate) fn kill_terminal(&self, id: &str) -> std::result::Result<(), EffectError> {
        self.port
            .as_ref()
            .ok_or_else(|| {
                EffectError::Unavailable("terminal capability is not configured".into())
            })?
            .kill_terminal(id)
    }

    fn authorizer(&self, session_id: &str) -> std::result::Result<Arc<Authorization>, EffectError> {
        let state = self.state.borrow();
        if state.session_id.as_deref() != Some(session_id) {
            return Err(EffectError::InvalidRequest(
                "request does not belong to the connected session".into(),
            ));
        }
        Ok(Arc::new(Authorization {
            generation: self.request_generation.unwrap_or(state.generation),
            state: self.state.subscribe(),
            perm_tx: self.perm_tx.clone(),
            event_tx: self.event_tx.clone(),
        }))
    }

    async fn execute(&self, request: EffectRequest) -> std::result::Result<EffectResponse, Error> {
        let port = self.port.as_ref().ok_or_else(|| {
            protocol_error(EffectError::Unavailable(
                "host effect capability is not configured".into(),
            ))
        })?;
        let capabilities = port.capabilities();
        let available = match &request {
            EffectRequest::ReadTextFile { .. } => capabilities.read_text_file,
            EffectRequest::WriteTextFile { .. } => capabilities.write_text_file,
            _ => capabilities.terminal,
        };
        if !available {
            return Err(protocol_error(EffectError::Unavailable(
                "requested capability is not configured".into(),
            )));
        }
        let authorizer = self
            .authorizer(request.session_id())
            .map_err(protocol_error)?;
        // Cleanup remains usable after cancellation; waiting and new effects do not.
        let cleanup = request.is_cleanup();
        if cleanup {
            return port
                .execute(request, authorizer)
                .await
                .map_err(protocol_error);
        }
        if authorizer.is_cancelled() {
            return Err(protocol_error(EffectError::Cancelled));
        }
        tokio::select! {
            biased;
            () = authorizer.cancelled() => Err(protocol_error(EffectError::Cancelled)),
            result = port.execute(request, authorizer.clone()) => result.map_err(protocol_error),
        }
    }

    pub(crate) async fn request_permission(
        &self,
        request: RequestPermissionRequest,
    ) -> RequestPermissionResponse {
        if request
            .tool_call
            .tool_call_id
            .to_string()
            .starts_with(HOST_EFFECT_PREFIX)
        {
            return RequestPermissionResponse::new(RequestPermissionOutcome::Cancelled);
        }
        let Ok(authorizer) = self.authorizer(&request.session_id.to_string()) else {
            return RequestPermissionResponse::new(RequestPermissionOutcome::Cancelled);
        };
        tokio::select! {
            biased;
            () = authorizer.cancelled() => RequestPermissionResponse::new(RequestPermissionOutcome::Cancelled),
            response = permission::request_permission(request, &self.perm_tx, &self.event_tx) => response,
        }
    }

    pub(crate) async fn read_text_file(
        &self,
        req: ReadTextFileRequest,
    ) -> std::result::Result<ReadTextFileResponse, Error> {
        match self
            .execute(EffectRequest::ReadTextFile {
                session_id: req.session_id.to_string(),
                path: path_string(req.path)?,
                line: req.line,
                limit: req.limit,
            })
            .await?
        {
            EffectResponse::Text(content) => Ok(ReadTextFileResponse::new(content)),
            _ => Err(invalid_response()),
        }
    }

    pub(crate) async fn write_text_file(
        &self,
        req: WriteTextFileRequest,
    ) -> std::result::Result<WriteTextFileResponse, Error> {
        match self
            .execute(EffectRequest::WriteTextFile {
                session_id: req.session_id.to_string(),
                path: path_string(req.path)?,
                content: req.content,
            })
            .await?
        {
            EffectResponse::Written => Ok(WriteTextFileResponse::new()),
            _ => Err(invalid_response()),
        }
    }

    pub(crate) async fn create_terminal(
        &self,
        req: CreateTerminalRequest,
    ) -> std::result::Result<CreateTerminalResponse, Error> {
        match self
            .execute(EffectRequest::CreateTerminal {
                session_id: req.session_id.to_string(),
                command: req.command,
                executable: None,
                args: req.args,
                env: req
                    .env
                    .into_iter()
                    .map(|var| (var.name, var.value))
                    .collect(),
                cwd: req.cwd.map(path_string).transpose()?,
                output_byte_limit: req.output_byte_limit,
            })
            .await?
        {
            EffectResponse::TerminalCreated(id) => Ok(CreateTerminalResponse::new(id)),
            _ => Err(invalid_response()),
        }
    }

    pub(crate) async fn terminal_output(
        &self,
        req: TerminalOutputRequest,
    ) -> std::result::Result<TerminalOutputResponse, Error> {
        match self
            .execute(EffectRequest::TerminalOutput {
                session_id: req.session_id.to_string(),
                terminal_id: req.terminal_id.to_string(),
            })
            .await?
        {
            EffectResponse::TerminalOutput {
                output,
                truncated,
                exit,
            } => {
                Ok(TerminalOutputResponse::new(output, truncated)
                    .exit_status(exit.map(exit_status)))
            }
            _ => Err(invalid_response()),
        }
    }

    pub(crate) async fn wait_for_terminal_exit(
        &self,
        req: WaitForTerminalExitRequest,
    ) -> std::result::Result<WaitForTerminalExitResponse, Error> {
        match self
            .execute(EffectRequest::WaitForTerminalExit {
                session_id: req.session_id.to_string(),
                terminal_id: req.terminal_id.to_string(),
            })
            .await?
        {
            EffectResponse::TerminalExited(exit) => {
                Ok(WaitForTerminalExitResponse::new(exit_status(exit)))
            }
            _ => Err(invalid_response()),
        }
    }

    pub(crate) async fn kill_terminal_request(
        &self,
        req: KillTerminalRequest,
    ) -> std::result::Result<KillTerminalResponse, Error> {
        match self
            .execute(EffectRequest::KillTerminal {
                session_id: req.session_id.to_string(),
                terminal_id: req.terminal_id.to_string(),
            })
            .await?
        {
            EffectResponse::TerminalKilled => Ok(KillTerminalResponse::new()),
            _ => Err(invalid_response()),
        }
    }

    pub(crate) async fn release_terminal(
        &self,
        req: ReleaseTerminalRequest,
    ) -> std::result::Result<ReleaseTerminalResponse, Error> {
        match self
            .execute(EffectRequest::ReleaseTerminal {
                session_id: req.session_id.to_string(),
                terminal_id: req.terminal_id.to_string(),
            })
            .await?
        {
            EffectResponse::TerminalReleased => Ok(ReleaseTerminalResponse::new()),
            _ => Err(invalid_response()),
        }
    }
}

struct Authorization {
    generation: u64,
    state: watch::Receiver<TurnState>,
    perm_tx: mpsc::UnboundedSender<AcpPermissionRequest>,
    event_tx: mpsc::UnboundedSender<Result<AcpEvent>>,
}

impl Authorization {
    async fn cancelled(&self) {
        let mut state = self.state.clone();
        loop {
            if self.is_cancelled() || state.changed().await.is_err() {
                return;
            }
        }
    }
}

impl EffectAuthorizer for Authorization {
    fn authorize(&self, request: EffectRequest) -> EffectFuture<'_, ()> {
        Box::pin(async move {
            if self.is_cancelled() {
                return Err(EffectError::Cancelled);
            }
            if self.state.borrow().session_id.as_deref() != Some(request.session_id()) {
                return Err(EffectError::InvalidRequest(
                    "authorization belongs to another session".into(),
                ));
            }
            let session_id = request.session_id().to_owned();
            let (title, kind, input) = match request {
                EffectRequest::ReadTextFile {
                    path, line, limit, ..
                } => (
                    "Read",
                    ToolKind::Read,
                    serde_json::json!({"file_path": path, "line": line, "limit": limit}),
                ),
                EffectRequest::WriteTextFile { path, content, .. } => (
                    "Write",
                    ToolKind::Edit,
                    serde_json::json!({"file_path": path, "content": content}),
                ),
                EffectRequest::CreateTerminal {
                    command,
                    executable,
                    args,
                    env,
                    cwd,
                    output_byte_limit,
                    ..
                } => {
                    let full_command = std::iter::once(&command)
                        .chain(args.iter())
                        .map(|arg| shell_word(arg))
                        .collect::<Vec<_>>()
                        .join(" ");
                    (
                        "Bash",
                        ToolKind::Execute,
                        serde_json::json!({"command": full_command, "executable": executable, "paths": executable.iter().collect::<Vec<_>>(), "args": args, "env": env, "cwd": cwd, "path": cwd, "output_byte_limit": output_byte_limit}),
                    )
                }
                _ => return Ok(()),
            };
            static REQUEST_ID: AtomicU64 = AtomicU64::new(0);
            let tool_call_id = format!(
                "{HOST_EFFECT_PREFIX}{}",
                REQUEST_ID.fetch_add(1, Ordering::Relaxed)
            );
            let request = RequestPermissionRequest::new(
                session_id,
                ToolCallUpdate::new(
                    tool_call_id,
                    ToolCallUpdateFields::new()
                        .title(title.to_owned())
                        .kind(kind)
                        .raw_input(input),
                ),
                vec![
                    PermissionOption::new("allow", "Allow", PermissionOptionKind::AllowOnce),
                    PermissionOption::new("reject", "Reject", PermissionOptionKind::RejectOnce),
                ],
            );
            let outcome = tokio::select! {
                biased;
                () = self.cancelled() => Err(EffectError::Cancelled),
                response = permission::request_permission(request, &self.perm_tx, &self.event_tx) => match response.outcome {
                    RequestPermissionOutcome::Selected(selected) if selected.option_id.to_string() == "allow" => Ok(()),
                    RequestPermissionOutcome::Cancelled => Err(EffectError::Cancelled),
                    _ => Err(EffectError::Denied("host policy refused the effect".into())),
                },
            };
            if self.is_cancelled() {
                Err(EffectError::Cancelled)
            } else {
                outcome
            }
        })
    }

    fn is_cancelled(&self) -> bool {
        let state = self.state.borrow();
        state.cancelled || state.generation != self.generation
    }
}

fn shell_word(word: &str) -> String {
    if !word.is_empty()
        && word
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_./-:=+".contains(&byte))
    {
        word.to_owned()
    } else {
        format!("'{}'", word.replace('\'', "'\"'\"'"))
    }
}

fn exit_status(exit: TerminalExit) -> TerminalExitStatus {
    TerminalExitStatus::new()
        .exit_code(exit.exit_code)
        .signal(exit.signal)
}

fn invalid_response() -> Error {
    Error::new(-32603, "effect port returned an incompatible response")
}

fn protocol_error(error: EffectError) -> Error {
    let code = match &error {
        EffectError::Denied(_) => -32003,
        EffectError::Unavailable(_) => -32601,
        EffectError::Cancelled => -32800,
        EffectError::InvalidRequest(_) => -32602,
        EffectError::Failed(_) => -32603,
    };
    let category = match &error {
        EffectError::Denied(_) => "denied",
        EffectError::Unavailable(_) => "unavailable",
        EffectError::Cancelled => "cancelled",
        EffectError::InvalidRequest(_) => "invalid_request",
        EffectError::Failed(_) => "failed",
    };
    Error::new(code, error.to_string()).data(serde_json::json!({"effect": category}))
}

fn path_string(path: std::path::PathBuf) -> std::result::Result<String, Error> {
    path.into_os_string().into_string().map_err(|_| {
        protocol_error(EffectError::InvalidRequest(
            "effect path is not UTF-8".into(),
        ))
    })
}

#[cfg(test)]
#[path = "effects.tests.rs"]
mod tests;
