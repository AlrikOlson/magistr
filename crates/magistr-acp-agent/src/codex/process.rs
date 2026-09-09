//! Multiplexed JSONL RPC: stdout keeps moving during approvals and steering.

use serde_json::{Value, json};
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::{Mutex as AsyncMutex, mpsc, oneshot};
use tokio::task::JoinHandle;

use super::frame::Frame;
use super::routing::Routing;
use crate::AgentError;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const STDERR_LIMIT: usize = 8192;

pub(crate) struct CodexProcess {
    _child: Child,
    writer: Arc<AsyncMutex<ChildStdin>>,
    routing: Arc<Mutex<Routing>>,
    next_id: AtomicU64,
    reader: JoinHandle<()>,
    stderr_reader: JoinHandle<()>,
}

impl CodexProcess {
    pub fn spawn(program: &Path, prefix: &[String]) -> Result<Self, AgentError> {
        let mut child = Command::new(program)
            .args(prefix)
            .args(["app-server", "--listen", "stdio://"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|error| {
                AgentError::provider(format!(
                    "cannot start Codex app-server at {}: {error}",
                    program.display()
                ))
            })?;
        let writer = Arc::new(AsyncMutex::new(
            child
                .stdin
                .take()
                .ok_or_else(|| AgentError::provider("Codex stdin missing"))?,
        ));
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AgentError::provider("Codex stdout missing"))?;
        let mut stderr = child
            .stderr
            .take()
            .ok_or_else(|| AgentError::provider("Codex stderr missing"))?;
        let stderr_tail = Arc::new(Mutex::new(Vec::<u8>::new()));
        let tail = Arc::clone(&stderr_tail);
        let stderr_reader = tokio::spawn(async move {
            let mut buffer = [0; 1024];
            while let Ok(count) = stderr.read(&mut buffer).await {
                if count == 0 {
                    break;
                }
                let mut tail = tail.lock().unwrap_or_else(PoisonError::into_inner);
                tail.extend_from_slice(&buffer[..count]);
                let excess = tail.len().saturating_sub(STDERR_LIMIT);
                tail.drain(..excess);
            }
        });
        let routing = Arc::new(Mutex::new(Routing::default()));
        let reader = tokio::spawn(read_stdout(
            stdout,
            Arc::clone(&writer),
            Arc::clone(&routing),
            stderr_tail,
        ));
        Ok(Self {
            _child: child,
            writer,
            routing,
            next_id: AtomicU64::new(1),
            reader,
            stderr_reader,
        })
    }

    pub async fn request(&self, method: &str, params: Value) -> Result<Value, AgentError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (send, receive) = oneshot::channel();
        {
            let mut routing = self.routing.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(error) = &routing.failure {
                return Err(AgentError::provider(error));
            }
            if method == "turn/start"
                && let Some(thread) = params["threadId"].as_str()
            {
                routing.active.insert(thread.into(), None);
                routing.starting_requests.insert(id, thread.into());
            }
            routing.pending.insert(id, send);
        }
        if let Err(error) = self
            .write(&json!({"id":id,"method":method,"params":params}))
            .await
        {
            self.routing
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .pending
                .remove(&id);
            return Err(error);
        }
        let response = tokio::time::timeout(REQUEST_TIMEOUT, receive).await;
        self.routing
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pending
            .remove(&id);
        match response {
            Ok(Ok(Ok(result))) => Ok(result),
            Ok(Ok(Err(error))) => Err(AgentError::provider(format!("Codex {method}: {error}"))),
            Ok(Err(_)) => Err(AgentError::provider(format!(
                "Codex disconnected during {method}"
            ))),
            Err(_) => Err(AgentError::provider(format!(
                "Codex {method} timed out after 30 seconds"
            ))),
        }
    }

    pub async fn write(&self, value: &Value) -> Result<(), AgentError> {
        if let Some(error) = self
            .routing
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .failure
            .clone()
        {
            return Err(AgentError::provider(error));
        }
        write_line(&self.writer, value).await.map_err(|error| {
            self.routing
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .fail(error.clone());
            AgentError::provider(error)
        })
    }

    pub fn has_active_turn(&self, thread: &str) -> bool {
        self.routing
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .active
            .contains_key(thread)
    }

    /// Permission relay must also stop when the runtime disappears, even
    /// when the ACP client has not answered its outstanding question.
    pub async fn failed(&self) -> String {
        let mut signal = self
            .routing
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .failure_signal
            .subscribe();
        loop {
            if let Some(error) = signal.borrow_and_update().clone() {
                return error;
            }
            if signal.changed().await.is_err() {
                return "Codex app-server disconnected".into();
            }
        }
    }

    /// Report a refused interrupt explicitly. Its acknowledgement alone does
    /// not complete cancellation: the caller still drains the matching terminal.
    pub async fn interrupt(&self, thread_id: &str, turn_id: &str) -> Result<(), AgentError> {
        self.request(
            "turn/interrupt",
            json!({"threadId":thread_id,"turnId":turn_id}),
        )
        .await
        .map(|_| ())
    }

    pub fn subscribe(
        &self,
        thread_id: &str,
    ) -> Result<mpsc::UnboundedReceiver<Result<Frame, String>>, AgentError> {
        let mut routing = self.routing.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(error) = &routing.failure {
            return Err(AgentError::provider(error));
        }
        if routing.threads.contains_key(thread_id) {
            return Err(AgentError::SessionExists(thread_id.into()));
        }
        let (send, receive) = mpsc::unbounded_channel();
        routing.threads.insert(thread_id.into(), send);
        Ok(receive)
    }
}

impl Drop for CodexProcess {
    fn drop(&mut self) {
        self.reader.abort();
        self.stderr_reader.abort();
        self.routing
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .fail("Codex app-server closed".into());
    }
}

async fn write_line(writer: &AsyncMutex<ChildStdin>, value: &Value) -> Result<(), String> {
    let mut bytes =
        serde_json::to_vec(value).map_err(|error| format!("encode Codex request: {error}"))?;
    bytes.push(b'\n');
    tokio::time::timeout(REQUEST_TIMEOUT, async {
        let mut writer = writer.lock().await;
        writer
            .write_all(&bytes)
            .await
            .map_err(|error| format!("write Codex app-server: {error}"))?;
        writer
            .flush()
            .await
            .map_err(|error| format!("flush Codex app-server: {error}"))
    })
    .await
    .map_err(|_| "Codex app-server stdin write timed out after 30 seconds".to_owned())?
}

async fn read_stdout(
    stdout: ChildStdout,
    writer: Arc<AsyncMutex<ChildStdin>>,
    routing: Arc<Mutex<Routing>>,
    stderr: Arc<Mutex<Vec<u8>>>,
) {
    let mut lines = BufReader::new(stdout).lines();
    let error = loop {
        let line = match lines.next_line().await {
            Ok(Some(line)) => line,
            Ok(None) => break "Codex app-server exited or disconnected".to_owned(),
            Err(error) => break format!("read Codex app-server: {error}"),
        };
        let frame = Frame::parse(&line);
        match frame {
            Frame::Malformed(error) => {
                break format!("malformed Codex app-server message: {error}");
            }
            Frame::Response { id, result } => {
                if let Some(id) = id.as_u64() {
                    let pending = {
                        let mut routing = routing.lock().unwrap_or_else(PoisonError::into_inner);
                        if let Some(thread) = routing.starting_requests.remove(&id)
                            && let Some(active) = routing.active.get_mut(&thread)
                        {
                            *active = result["turn"]["id"].as_str().map(str::to_owned);
                        }
                        routing.pending.remove(&id)
                    };
                    if let Some(pending) = pending {
                        let _ = pending.send(Ok(result));
                    }
                }
            }
            Frame::Error { id, error } => {
                if let Some(id) = id.as_u64() {
                    let pending = {
                        let mut routing = routing.lock().unwrap_or_else(PoisonError::into_inner);
                        if let Some(thread) = routing.starting_requests.remove(&id) {
                            routing.active.remove(&thread);
                        }
                        routing.pending.remove(&id)
                    };
                    if let Some(pending) = pending {
                        let _ = pending.send(Err(error.to_string()));
                    }
                }
            }
            frame => {
                if let Frame::Notification { method, params } = &frame
                    && method == "account/updated"
                    && params["authMode"].as_str() != Some("chatgpt")
                {
                    break "Codex runtime ChatGPT login changed or expired; API billing fallback is disabled".into();
                }
                if requires_turn_route(&frame)
                    && (frame.thread_id().is_none_or(str::is_empty)
                        || frame.turn_id().is_none_or(str::is_empty))
                {
                    break "malformed Codex turn message: missing threadId or turnId".into();
                }
                if let Frame::Notification { method, .. } = &frame
                    && let Some(thread) = frame.thread_id()
                {
                    let mut routing = routing.lock().unwrap_or_else(PoisonError::into_inner);
                    if method == "turn/started" {
                        if let Some(active) = routing.active.get_mut(thread) {
                            *active = frame.turn_id().map(str::to_owned);
                        }
                    } else if method == "turn/completed"
                        && routing.active.get(thread).and_then(Option::as_deref) == frame.turn_id()
                    {
                        routing.active.remove(thread);
                    }
                }
                let receiver = frame.thread_id().and_then(|id| {
                    routing
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .threads
                        .get(id)
                        .cloned()
                });
                if let Frame::Request { id, method, params } = &frame {
                    let approval = matches!(
                        method.as_str(),
                        "item/commandExecution/requestApproval" | "item/fileChange/requestApproval"
                    );
                    if !approval
                        || receiver.is_none()
                        || params["itemId"].as_str().is_none()
                        || frame.turn_id().is_none()
                    {
                        let response = json!({"id":id,"error":{"code":-32601,"message":format!("Magistr cannot serve Codex request {method} in this context")}});
                        if let Err(error) = write_line(&writer, &response).await {
                            break error;
                        }
                        continue;
                    }
                    let active = frame.thread_id().and_then(|thread| {
                        routing
                            .lock()
                            .unwrap_or_else(PoisonError::into_inner)
                            .active
                            .get(thread)
                            .cloned()
                    });
                    if !active.is_some_and(|active| {
                        active
                            .as_deref()
                            .is_none_or(|id| Some(id) == frame.turn_id())
                    }) {
                        if let Err(error) =
                            write_line(&writer, &json!({"id":id,"result":{"decision":"cancel"}}))
                                .await
                        {
                            break error;
                        }
                        continue;
                    }
                }
                if let Some(receiver) = receiver
                    && let Err(failed) = receiver.send(Ok(frame))
                    && let Ok(Frame::Request { id, .. }) = failed.0
                {
                    let _ =
                        write_line(&writer, &json!({"id":id,"result":{"decision":"cancel"}})).await;
                }
            }
        }
    };
    let stderr = stderr.lock().unwrap_or_else(PoisonError::into_inner);
    let note = String::from_utf8_lossy(&stderr);
    let error = if note.trim().is_empty() {
        error
    } else {
        format!("{error}; stderr: {}", note.trim())
    };
    routing
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .fail(error);
}

fn requires_turn_route(frame: &Frame) -> bool {
    let method = match frame {
        Frame::Request { method, .. } | Frame::Notification { method, .. } => method.as_str(),
        _ => return false,
    };
    matches!(
        method,
        "turn/started"
            | "turn/completed"
            | "turn/plan/updated"
            | "item/started"
            | "item/completed"
            | "item/agentMessage/delta"
            | "item/reasoning/textDelta"
            | "item/reasoning/summaryTextDelta"
            | "item/commandExecution/outputDelta"
            | "thread/tokenUsage/updated"
    )
}
