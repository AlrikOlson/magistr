use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use agent_client_protocol::schema::v1::{
    CreateTerminalRequest, EnvVariable, ReadTextFileRequest, ReleaseTerminalRequest,
    TerminalOutputRequest, WaitForTerminalExitRequest,
};
use magistr_application::effects::{
    EffectAuthorizer, EffectCapabilities, EffectError, EffectFuture, EffectPort, EffectRequest,
    EffectResponse, SourceRoute, TerminalExit,
};
use tokio::sync::mpsc;

use super::resolve_target;
use crate::handlers::{fs, terminal};
use crate::{AcpEvent, Result};

/// Explicit local host adapter. It owns its terminals and authorizes resolved
/// resources through the supplied session decision path before effect admission.
/// It does not isolate the process from the host operating system.
///
/// Source writes have exactly one route, the host's [`SourceRoute`]: an
/// admitted workspace's pipeline — journaled and receipted — or a typed
/// refusal. A port composed with a refusal advertises no write capability
/// and answers any write it is nevertheless asked for with the refusal
/// before resolution or authorization: nothing is written directly, ever.
/// On the admitted route authorization comes first: a denied or cancelled
/// write never reaches the pipeline. The `fs_write` event names the route.
pub struct LocalEffectPort {
    root: PathBuf,
    events: mpsc::UnboundedSender<Result<AcpEvent>>,
    source: SourceRoute,
    terminals: terminal::TerminalManager,
    owners: Mutex<HashMap<String, String>>,
    admission: Mutex<()>,
    closed: AtomicBool,
}

impl LocalEffectPort {
    /// Configure a workspace root, the owning session's event channel and
    /// the route the session's authorized source writes take.
    #[must_use]
    pub fn new(
        root: PathBuf,
        events: mpsc::UnboundedSender<Result<AcpEvent>>,
        source: SourceRoute,
    ) -> Self {
        Self {
            root,
            events,
            source,
            terminals: terminal::new_terminal_manager(),
            owners: Mutex::new(HashMap::new()),
            admission: Mutex::new(()),
            closed: AtomicBool::new(false),
        }
    }

    /// The refusal a write would get from this port, when it has one.
    fn refused_write(&self, request: &EffectRequest) -> Option<EffectError> {
        let EffectRequest::WriteTextFile { path, .. } = request else {
            return None;
        };
        let refusal = self.source.refusal()?;
        let _ = self.events.send(Ok(AcpEvent::System {
            subtype: "fs_write".to_string(),
            data: serde_json::json!({
                "path": path,
                "route": "refused",
                "refusal": refusal.kind(),
                "reason": refusal.to_string(),
            }),
        }));
        Some(refusal.effect_error())
    }

    fn workspace_path(&self, path: &str) -> std::result::Result<String, EffectError> {
        if !Path::new(path).is_absolute() {
            return Err(EffectError::InvalidRequest(
                "ACP resource paths must be absolute".into(),
            ));
        }
        let root = resolve_target(&self.root, &self.root)?;
        let target = resolve_target(&root, Path::new(path))?;
        if !target.starts_with(&root) {
            return Err(EffectError::Denied(
                "resource is outside the configured workspace".into(),
            ));
        }
        path_text(&target)
    }

    fn resolve(
        &self,
        mut request: EffectRequest,
    ) -> std::result::Result<EffectRequest, EffectError> {
        match &mut request {
            EffectRequest::ReadTextFile { path, .. }
            | EffectRequest::WriteTextFile { path, .. } => {
                *path = self.workspace_path(path)?;
            }
            EffectRequest::CreateTerminal {
                command,
                executable,
                cwd,
                env,
                ..
            } => {
                let directory = cwd.clone().unwrap_or(path_text(&self.root)?);
                let directory = self.workspace_path(&directory)?;
                if !Path::new(&directory).is_dir() {
                    return Err(EffectError::InvalidRequest(
                        "terminal cwd must be an existing directory".into(),
                    ));
                }
                *executable = Some(resolve_command(command, env, Path::new(&directory))?);
                *cwd = Some(directory);
            }
            _ => {}
        }
        Ok(request)
    }

    fn owns(&self, session: &str, id: &str) -> std::result::Result<(), EffectError> {
        let owners = self.owners.lock().unwrap_or_else(|p| p.into_inner());
        match owners.get(id) {
            Some(owner) if owner == session => Ok(()),
            Some(_) => Err(EffectError::Denied(
                "terminal belongs to another session".into(),
            )),
            None => Err(EffectError::InvalidRequest(
                "unknown or released terminal".into(),
            )),
        }
    }

    async fn perform(
        &self,
        request: EffectRequest,
        authorizer: &dyn EffectAuthorizer,
    ) -> std::result::Result<EffectResponse, EffectError> {
        match request {
            EffectRequest::ReadTextFile {
                session_id,
                path,
                line,
                limit,
            } => {
                let response = fs::read_text_file(
                    ReadTextFileRequest::new(session_id, path)
                        .line(line)
                        .limit(limit),
                    &self.events,
                )
                .await
                .map_err(protocol_error)?;
                Ok(EffectResponse::Text(response.content))
            }
            EffectRequest::WriteTextFile {
                session_id: _,
                path,
                content,
            } => {
                let SourceRoute::Admitted(writer) = &self.source else {
                    // Refused ahead of authorization in `execute`; a direct
                    // `perform` gets the same answer and no write.
                    return Err(self
                        .source
                        .refusal()
                        .map(magistr_application::effects::SourceRefusal::effect_error)
                        .unwrap_or_else(|| EffectError::Unavailable("no source route".into())));
                };
                let receipt = writer.write_text(&path, &content).await?;
                let _ = self.events.send(Ok(AcpEvent::System {
                    subtype: "fs_write".to_string(),
                    data: serde_json::json!({
                        "path": path,
                        "route": "workspace",
                        "journal_seq": receipt.journal_seq,
                        "revision": receipt.revision,
                        "changed": receipt.changed,
                    }),
                }));
                Ok(EffectResponse::Written)
            }
            EffectRequest::CreateTerminal {
                session_id,
                command: _,
                executable,
                args,
                env,
                cwd,
                output_byte_limit,
            } => {
                let _admission = self.admission.lock().unwrap_or_else(|p| p.into_inner());
                if self.closed.load(Ordering::SeqCst) || authorizer.is_cancelled() {
                    return Err(EffectError::Cancelled);
                }
                let request = CreateTerminalRequest::new(
                    session_id.clone(),
                    executable.ok_or_else(|| {
                        EffectError::InvalidRequest("unresolved executable".into())
                    })?,
                )
                .args(args)
                .env(
                    env.into_iter()
                        .map(|(name, value)| EnvVariable::new(name, value))
                        .collect(),
                )
                .cwd(cwd.map(PathBuf::from))
                .output_byte_limit(output_byte_limit);
                let response = terminal::create_terminal(request, &self.terminals, &self.events)
                    .map_err(protocol_error)?;
                let id = response.terminal_id.to_string();
                self.owners
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .insert(id.clone(), session_id);
                Ok(EffectResponse::TerminalCreated(id))
            }
            EffectRequest::TerminalOutput {
                session_id,
                terminal_id,
            } => {
                self.owns(&session_id, &terminal_id)?;
                let response = terminal::terminal_output(
                    &TerminalOutputRequest::new(session_id, terminal_id),
                    &self.terminals,
                )
                .map_err(protocol_error)?;
                Ok(EffectResponse::TerminalOutput {
                    output: response.output,
                    truncated: response.truncated,
                    exit: response.exit_status.map(|exit| TerminalExit {
                        exit_code: exit.exit_code,
                        signal: exit.signal,
                    }),
                })
            }
            EffectRequest::WaitForTerminalExit {
                session_id,
                terminal_id,
            } => {
                self.owns(&session_id, &terminal_id)?;
                let response = terminal::wait_for_terminal_exit(
                    &WaitForTerminalExitRequest::new(session_id, terminal_id),
                    &self.terminals,
                )
                .await
                .map_err(protocol_error)?;
                Ok(EffectResponse::TerminalExited(TerminalExit {
                    exit_code: response.exit_status.exit_code,
                    signal: response.exit_status.signal,
                }))
            }
            EffectRequest::KillTerminal {
                session_id,
                terminal_id,
            } => {
                self.owns(&session_id, &terminal_id)?;
                terminal::kill_terminal(
                    &agent_client_protocol::schema::v1::KillTerminalRequest::new(
                        session_id,
                        terminal_id,
                    ),
                    &self.terminals,
                )
                .map_err(protocol_error)?;
                Ok(EffectResponse::TerminalKilled)
            }
            EffectRequest::ReleaseTerminal {
                session_id,
                terminal_id,
            } => {
                self.owns(&session_id, &terminal_id)?;
                terminal::release_terminal(
                    &ReleaseTerminalRequest::new(session_id, terminal_id.clone()),
                    &self.terminals,
                    &self.events,
                )
                .map_err(protocol_error)?;
                self.owners
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .remove(&terminal_id);
                self.terminals
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .remove(&terminal_id);
                Ok(EffectResponse::TerminalReleased)
            }
        }
    }
}

impl EffectPort for LocalEffectPort {
    fn capabilities(&self) -> EffectCapabilities {
        EffectCapabilities {
            read_text_file: true,
            write_text_file: self.source.is_admitted(),
            terminal: true,
        }
    }

    fn execute(
        &self,
        request: EffectRequest,
        authorizer: Arc<dyn EffectAuthorizer>,
    ) -> EffectFuture<'_, EffectResponse> {
        Box::pin(async move {
            if self.closed.load(Ordering::SeqCst) {
                return Err(EffectError::Cancelled);
            }
            if let Some(refused) = self.refused_write(&request) {
                return Err(refused);
            }
            let needs_authorization = matches!(
                request,
                EffectRequest::ReadTextFile { .. }
                    | EffectRequest::WriteTextFile { .. }
                    | EffectRequest::CreateTerminal { .. }
            );
            if !request.is_cleanup() && authorizer.is_cancelled() {
                return Err(EffectError::Cancelled);
            }
            let resolved = self.resolve(request.clone())?;
            if needs_authorization {
                authorizer.authorize(resolved.clone()).await?;
                if authorizer.is_cancelled() {
                    return Err(EffectError::Cancelled);
                }
                if self.resolve(request)? != resolved {
                    return Err(EffectError::Denied(
                        "resource changed while authorization was pending; retry required".into(),
                    ));
                }
            }
            if !resolved.is_cleanup() && authorizer.is_cancelled() {
                return Err(EffectError::Cancelled);
            }
            if self.closed.load(Ordering::SeqCst) {
                return Err(EffectError::Cancelled);
            }
            self.perform(resolved, authorizer.as_ref()).await
        })
    }

    fn kill_terminal(&self, terminal_id: &str) -> std::result::Result<(), EffectError> {
        terminal::kill(&self.terminals, terminal_id).map_err(protocol_error)
    }

    fn close(&self) {
        let _admission = self.admission.lock().unwrap_or_else(|p| p.into_inner());
        self.closed.store(true, Ordering::SeqCst);
        self.owners
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clear();
        self.terminals
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clear();
    }
}

impl Drop for LocalEffectPort {
    fn drop(&mut self) {
        self.close();
    }
}

fn path_text(path: &Path) -> std::result::Result<String, EffectError> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| EffectError::InvalidRequest("path is not UTF-8".into()))
}

fn resolve_command(
    command: &str,
    env: &[(String, String)],
    cwd: &Path,
) -> std::result::Result<String, EffectError> {
    let command_path = Path::new(command);
    let candidate = if command_path.components().count() > 1 || command_path.is_absolute() {
        executable_candidate(resolve_target(cwd, command_path)?)
            .ok_or_else(|| EffectError::Failed(format!("executable not found: {command}")))?
    } else {
        let search = env
            .iter()
            .rev()
            .find(|(name, _)| name == "PATH" || cfg!(windows) && name.eq_ignore_ascii_case("PATH"))
            .map(|(_, value)| std::ffi::OsString::from(value))
            .or_else(|| std::env::var_os("PATH"))
            .unwrap_or_default();
        std::env::split_paths(&search)
            .map(|directory| {
                let directory = if directory.is_absolute() {
                    directory
                } else {
                    cwd.join(directory)
                };
                directory.join(command)
            })
            .find_map(executable_candidate)
            .ok_or_else(|| EffectError::Failed(format!("executable not found: {command}")))?
    };
    let resolved = resolve_target(cwd, &candidate)?;
    if !is_executable(&resolved) {
        return Err(EffectError::Failed(format!(
            "not an executable file: {}",
            resolved.display()
        )));
    }
    path_text(&resolved)
}

// Rust permits omitting .exe on Windows. Other extensions must be explicit.
fn executable_candidate(path: PathBuf) -> Option<PathBuf> {
    #[cfg(windows)]
    if path.extension().is_none() {
        let executable = path.with_extension("exe");
        if is_executable(&executable) {
            return Some(executable);
        }
    }
    is_executable(&path).then_some(path)
}

fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn protocol_error(error: agent_client_protocol::Error) -> EffectError {
    EffectError::Failed(error.to_string())
}

#[cfg(test)]
#[path = "local_effect_port.tests.rs"]
mod tests;
