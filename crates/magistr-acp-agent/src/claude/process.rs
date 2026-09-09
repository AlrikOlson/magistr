//! One `claude` process: its stdin, its stdout as frames, and the control
//! requests in flight.

use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as SyncMutex, PoisonError};
use std::time::Duration;

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{Mutex, mpsc, oneshot};

use crate::AgentError;
use crate::claude::control::ControlRequest;
use crate::claude::frame::{ControlResponseFrame, Frame};

/// How many stderr lines are kept for an error message.
const STDERR_TAIL: usize = 20;
/// How long a control request may wait for its response.
const CONTROL_TIMEOUT: Duration = Duration::from_secs(60);
/// How long `close` waits for the process to exit after its stdin closes.
const EXIT_GRACE: Duration = Duration::from_secs(5);

type Pending = Arc<SyncMutex<HashMap<String, oneshot::Sender<ControlResponseFrame>>>>;

/// A running Claude Code process. A reader task turns its stdout into
/// [`Frame`]s: a `control_response` for a request in flight resolves that
/// request, everything else queues for [`next_frame`](Self::next_frame).
/// Writes go through [`write`](Self::write) one line at a time. Dropping it
/// kills the process.
pub(crate) struct ClaudeCli {
    child: Mutex<Child>,
    stdin: Mutex<Option<ChildStdin>>,
    frames: Mutex<mpsc::UnboundedReceiver<Frame>>,
    pending: Pending,
    stderr: Arc<SyncMutex<VecDeque<String>>>,
    counter: AtomicU64,
}

impl ClaudeCli {
    /// Spawn `program` with `args` in `cwd`.
    pub fn spawn(program: &Path, args: &[String], cwd: &Path) -> Result<Self, AgentError> {
        let mut child = Command::new(program)
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| {
                AgentError::provider(format!("failed to spawn {}: {e}", program.display()))
            })?;
        let stdin = child.stdin.take();
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AgentError::provider("claude has no stdout"))?;
        let stderr = child.stderr.take();

        let (tx, rx) = mpsc::unbounded_channel();
        let pending: Pending = Arc::default();
        let tail: Arc<SyncMutex<VecDeque<String>>> = Arc::default();

        tokio::spawn({
            let pending = Arc::clone(&pending);
            async move {
                let mut lines = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if line.trim().is_empty() {
                        continue;
                    }
                    match Frame::parse(&line) {
                        Frame::ControlResponse(response) => {
                            let waiting = pending
                                .lock()
                                .unwrap_or_else(PoisonError::into_inner)
                                .remove(&response.request_id);
                            match waiting {
                                Some(sender) => {
                                    let _ = sender.send(response);
                                }
                                None => {
                                    if tx.send(Frame::ControlResponse(response)).is_err() {
                                        break;
                                    }
                                }
                            }
                        }
                        frame => {
                            if tx.send(frame).is_err() {
                                break;
                            }
                        }
                    }
                }
            }
        });
        if let Some(stderr) = stderr {
            tokio::spawn({
                let tail = Arc::clone(&tail);
                async move {
                    let mut lines = BufReader::new(stderr).lines();
                    while let Ok(Some(line)) = lines.next_line().await {
                        tracing::debug!(target: "magistr_acp_agent::claude", "{line}");
                        let mut tail = tail.lock().unwrap_or_else(PoisonError::into_inner);
                        if tail.len() == STDERR_TAIL {
                            tail.pop_front();
                        }
                        tail.push_back(line);
                    }
                }
            });
        }

        Ok(Self {
            child: Mutex::new(child),
            stdin: Mutex::new(stdin),
            frames: Mutex::new(rx),
            pending,
            stderr: tail,
            counter: AtomicU64::new(1),
        })
    }

    /// Write one frame as one line.
    pub async fn write(&self, frame: &Value) -> Result<(), AgentError> {
        let mut line = frame.to_string();
        line.push('\n');
        let mut stdin = self.stdin.lock().await;
        let Some(stdin) = stdin.as_mut() else {
            return Err(AgentError::provider("claude's stdin is closed"));
        };
        stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| AgentError::provider(format!("writing to claude failed: {e}")))?;
        stdin
            .flush()
            .await
            .map_err(|e| AgentError::provider(format!("writing to claude failed: {e}")))
    }

    /// Send a control request and wait for its response.
    pub async fn control(
        &self,
        request: ControlRequest,
    ) -> Result<ControlResponseFrame, AgentError> {
        let request_id = format!("magistr-{}", self.counter.fetch_add(1, Ordering::SeqCst));
        let (tx, rx) = oneshot::channel();
        self.pending
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(request_id.clone(), tx);
        if let Err(e) = self.write(&request.frame(&request_id)).await {
            self.pending
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .remove(&request_id);
            return Err(e);
        }
        match tokio::time::timeout(CONTROL_TIMEOUT, rx).await {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(_)) => Err(AgentError::provider(format!(
                "claude exited before answering {request:?}{}",
                self.stderr_note()
            ))),
            Err(_) => {
                self.pending
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .remove(&request_id);
                Err(AgentError::provider(format!(
                    "claude did not answer {request:?} in time"
                )))
            }
        }
    }

    /// Send a control request without waiting for its response; the
    /// response arrives as a frame and is ignored.
    pub async fn control_and_forget(&self, request: ControlRequest) -> Result<(), AgentError> {
        let request_id = format!("magistr-{}", self.counter.fetch_add(1, Ordering::SeqCst));
        self.write(&request.frame(&request_id)).await
    }

    /// The next frame the process wrote, or `None` once its stdout closed.
    pub async fn next_frame(&self) -> Option<Frame> {
        self.frames.lock().await.recv().await
    }

    /// The last lines the process wrote to stderr, for an error message.
    pub fn stderr_note(&self) -> String {
        let tail = self.stderr.lock().unwrap_or_else(PoisonError::into_inner);
        if tail.is_empty() {
            String::new()
        } else {
            format!(
                "; stderr: {}",
                tail.iter().cloned().collect::<Vec<_>>().join(" | ")
            )
        }
    }

    /// Close stdin (the CLI's graceful shutdown) and wait briefly for the
    /// exit; the process is killed if it lingers.
    pub async fn close(&self) {
        self.stdin.lock().await.take();
        let mut child = self.child.lock().await;
        if tokio::time::timeout(EXIT_GRACE, child.wait())
            .await
            .is_err()
        {
            let _ = child.kill().await;
        }
    }
}
