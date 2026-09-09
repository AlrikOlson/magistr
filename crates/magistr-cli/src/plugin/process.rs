//! Plugin subprocess management — spawn and communicate with executable plugins.
//!
//! Each executable plugin runs as a child process. Communication happens via
//! JSON-line protocol over stdin (requests) and stdout (responses). Each
//! JSON message is a single line terminated by `\n`.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::types::{PluginError, PluginRequest, PluginResponse};

/// A running plugin subprocess.
///
/// Owns the child process and provides async send/receive over the JSON-line
/// protocol. The process is killed on drop.
pub struct PluginProcess {
    /// Sender for writing JSON requests to the child's stdin.
    stdin_tx: mpsc::UnboundedSender<String>,
    /// Receiver for reading JSON responses from the child's stdout.
    response_rx: mpsc::UnboundedReceiver<Result<PluginResponse, PluginError>>,
    /// Handle to the child process (killed on drop).
    child: Child,
    /// Monotonically increasing request ID counter.
    next_id: AtomicU64,
    /// The plugin name (for error messages).
    plugin_name: String,
    /// Path to the plugin directory.
    plugin_dir: PathBuf,
    /// Handle to the stdin writer task (aborted on drop).
    _stdin_task: JoinHandle<()>,
    /// Handle to the stdout reader task (aborted on drop).
    _reader_task: JoinHandle<()>,
}

impl PluginProcess {
    /// Spawn a new plugin subprocess.
    ///
    /// The `entrypoint` is resolved relative to `plugin_dir`. The child process
    /// inherits stderr (for logging) but stdin/stdout are piped for protocol use.
    pub async fn spawn(
        plugin_name: &str,
        plugin_dir: &Path,
        entrypoint: &str,
    ) -> Result<Self, PluginError> {
        let entrypoint_path = plugin_dir.join(entrypoint);

        if !entrypoint_path.exists() {
            return Err(PluginError::ProcessStart(format!(
                "entrypoint not found: {}",
                entrypoint_path.display()
            )));
        }

        let mut child = Command::new(&entrypoint_path)
            .current_dir(plugin_dir)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| {
                PluginError::ProcessStart(format!(
                    "failed to spawn {}: {e}",
                    entrypoint_path.display()
                ))
            })?;

        let stdin = child.stdin.take().ok_or_else(|| {
            PluginError::ProcessStart("failed to capture plugin stdin".to_string())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            PluginError::ProcessStart("failed to capture plugin stdout".to_string())
        })?;

        // Spawn stdin writer task
        let (stdin_tx, mut stdin_rx) = mpsc::unbounded_channel::<String>();
        let stdin_task = tokio::spawn(async move {
            let mut stdin = stdin;
            while let Some(line) = stdin_rx.recv().await {
                if stdin.write_all(line.as_bytes()).await.is_err() {
                    break;
                }
                if stdin.write_all(b"\n").await.is_err() {
                    break;
                }
                if stdin.flush().await.is_err() {
                    break;
                }
            }
        });

        // Spawn stdout reader task
        let (response_tx, response_rx) = mpsc::unbounded_channel();
        let reader_plugin_name = plugin_name.to_string();
        let reader_task = tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let line = line.trim().to_string();
                if line.is_empty() {
                    continue;
                }
                let result = serde_json::from_str::<PluginResponse>(&line).map_err(|e| {
                    PluginError::ProcessIo(format!(
                        "invalid JSON from plugin '{}': {e} (line: {line})",
                        reader_plugin_name
                    ))
                });
                if response_tx.send(result).is_err() {
                    break;
                }
            }
        });

        Ok(Self {
            stdin_tx,
            response_rx,
            child,
            next_id: AtomicU64::new(1),
            plugin_name: plugin_name.to_string(),
            plugin_dir: plugin_dir.to_path_buf(),
            _stdin_task: stdin_task,
            _reader_task: reader_task,
        })
    }

    /// Allocate the next request ID.
    pub fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Send a request to the plugin subprocess.
    pub fn send(&self, request: &PluginRequest) -> Result<(), PluginError> {
        let json = serde_json::to_string(request).map_err(|e| {
            PluginError::ProcessIo(format!(
                "failed to serialize request for '{}': {e}",
                self.plugin_name
            ))
        })?;
        self.stdin_tx.send(json).map_err(|_| {
            PluginError::ProcessExited(format!("plugin '{}' stdin closed", self.plugin_name))
        })
    }

    /// Receive the next response from the plugin subprocess.
    ///
    /// Returns `None` if the plugin process has exited and the channel is drained.
    pub async fn recv(&mut self) -> Option<Result<PluginResponse, PluginError>> {
        self.response_rx.recv().await
    }

    /// Send a request and wait for the matching response (by ID).
    ///
    /// Non-matching responses received while waiting are dropped. Use this
    /// for simple request-response exchanges (init, activate, deactivate).
    pub async fn request(
        &mut self,
        request: &PluginRequest,
    ) -> Result<PluginResponse, PluginError> {
        let expected_id = request.id();
        self.send(request)?;

        let timeout = tokio::time::Duration::from_secs(30);
        let result = tokio::time::timeout(timeout, async {
            while let Some(result) = self.recv().await {
                let response = result?;
                if response.id() == expected_id {
                    return Ok(response);
                }
                // Non-matching response — skip
            }
            Err(PluginError::ProcessExited(format!(
                "plugin '{}' exited before responding to request {expected_id}",
                self.plugin_name
            )))
        })
        .await;

        match result {
            Ok(inner) => inner,
            Err(_) => Err(PluginError::Timeout(format!(
                "plugin '{}' did not respond within {timeout:?}",
                self.plugin_name
            ))),
        }
    }

    /// Get the plugin name.
    #[must_use]
    pub fn plugin_name(&self) -> &str {
        &self.plugin_name
    }

    /// Get the plugin directory path.
    #[must_use]
    pub fn plugin_dir(&self) -> &Path {
        &self.plugin_dir
    }

    /// Kill the plugin process.
    pub fn kill(&mut self) {
        // kill_on_drop will handle this, but explicit kill is cleaner
        let _ = self.child.start_kill();
    }

    /// Check if the plugin process is still running.
    pub fn try_wait(&mut self) -> Option<std::process::ExitStatus> {
        self.child.try_wait().ok().flatten()
    }
}

impl std::fmt::Debug for PluginProcess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginProcess")
            .field("plugin_name", &self.plugin_name)
            .field("plugin_dir", &self.plugin_dir)
            .finish_non_exhaustive()
    }
}

impl Drop for PluginProcess {
    fn drop(&mut self) {
        self.kill();
    }
}

#[cfg(all(test, unix))]
#[path = "process.tests.rs"]
mod tests;
