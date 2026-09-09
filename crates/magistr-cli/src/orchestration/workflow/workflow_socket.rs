//! Workflow guard socket — lightweight Unix socket server for cross-process
//! workflow state queries.
//!
//! The TUI process (`magistr next`) starts a socket listener that exposes
//! the in-memory `WorkflowTracker` state. The MCP server process
//! (`magistr mcp-serve`) queries this socket to enforce workflow guards
//! (e.g., research guard blocks Research→Plan without web research).
//!
//! This replaces the brittle file-based `.workflow-evidence.json` IPC.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicU32, Ordering};

/// Socket file name within `.magistr/` for workflow guard queries.
const SOCKET_NAME: &str = "workflow-guard.sock";

/// Shared workflow state exposed to the socket listener via atomics.
///
/// Updated by the TUI handler as it observes tool calls on the ACP stream.
/// Read by the socket listener to answer guard queries from the MCP server.
#[derive(Debug, Clone)]
pub struct SharedGuardState {
    /// Number of research tool calls observed (web_search, firecrawl, etc.).
    pub research_calls: Arc<AtomicU32>,
    /// Number of exploration tool calls observed (scope_read, Grep, etc.).
    pub exploration_calls: Arc<AtomicU32>,
    /// Current workflow phase as a u8 (WorkflowPhase::step_index).
    pub phase: Arc<AtomicU8>,
}

impl SharedGuardState {
    /// Create a new shared state with all counters at zero, phase at Researching.
    pub fn new() -> Self {
        Self {
            research_calls: Arc::new(AtomicU32::new(0)),
            exploration_calls: Arc::new(AtomicU32::new(0)),
            phase: Arc::new(AtomicU8::new(0)),
        }
    }

    /// Record a research tool call (web_search, brave_*, firecrawl, etc.).
    pub fn record_research(&self) {
        self.research_calls.fetch_add(1, Ordering::Relaxed);
    }

    /// Record an exploration tool call (scope_read, Grep, etc.).
    pub fn record_exploration(&self) {
        self.exploration_calls.fetch_add(1, Ordering::Relaxed);
    }

    /// Update the current phase index.
    pub fn set_phase(&self, step_index: usize) {
        self.phase.store(step_index as u8, Ordering::Relaxed);
    }

    /// Check if a phase transition is allowed.
    ///
    /// Currently only enforces: Research→Plan requires research_calls > 0.
    pub fn can_transition(&self, to_phase: &str) -> GuardResult {
        let current_phase = self.phase.load(Ordering::Relaxed);
        let research_count = self.research_calls.load(Ordering::Relaxed);

        // Research guard: block Research(0)→Plan(1+) if no research tools called
        if current_phase == 0 && to_phase != "research" && research_count == 0 {
            return GuardResult {
                allowed: false,
                reason: Some(
                    "No research tool calls detected. The Research phase requires at least one \
                     web research tool call before transitioning to the Plan phase. Use any \
                     available research tool: brave_web_search, brave_news_search (Brave Search MCP), \
                     context7 (library docs MCP), firecrawl (web scraping MCP), WebSearch/WebFetch \
                     (built-in), or any other web research MCP tool. Research best practices, \
                     API docs, known pitfalls, and idiomatic patterns for what you're about to build."
                        .to_string(),
                ),
            };
        }

        GuardResult {
            allowed: true,
            reason: None,
        }
    }

    /// Reset all counters for a new iteration.
    pub fn reset(&self) {
        self.research_calls.store(0, Ordering::Relaxed);
        self.exploration_calls.store(0, Ordering::Relaxed);
        self.phase.store(0, Ordering::Relaxed);
    }
}

impl Default for SharedGuardState {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of a guard check.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct GuardResult {
    pub allowed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Compute the socket path within the project's `.magistr/` directory.
pub fn guard_socket_path(project_dir: &Path) -> PathBuf {
    project_dir.join(".magistr").join(SOCKET_NAME)
}

/// Start the workflow guard socket listener.
///
/// Spawns a tokio task that listens on `.magistr/workflow-guard.sock` and
/// answers JSON queries from the MCP server process. Returns a handle that
/// cleans up the socket file on drop.
///
/// Unix domain sockets only: where there are none (Windows) this returns
/// `None` and the guard falls back to its permissive non-workflow mode.
///
/// # Protocol
///
/// Request (one JSON line):
/// ```json
/// {"query": "can_transition", "to": "plan"}
/// ```
///
/// Response (one JSON line):
/// ```json
/// {"allowed": false, "reason": "..."}
/// ```
#[cfg(unix)]
pub fn start_listener(project_dir: &Path, state: SharedGuardState) -> Option<GuardSocketHandle> {
    use tokio::net::UnixListener;

    let socket_path = guard_socket_path(project_dir);

    // Ensure .magistr/ exists
    let parent = socket_path.parent()?;
    std::fs::create_dir_all(parent).ok()?;

    // Remove stale socket from previous run
    let _ = std::fs::remove_file(&socket_path);

    // Bind — if this fails (permissions, path too long), skip silently.
    // The guard is a best-effort enhancement, not critical path.
    let listener = match UnixListener::bind(&socket_path) {
        Ok(l) => l,
        Err(e) => {
            tracing::warn!("workflow guard socket bind failed: {e}");
            return None;
        }
    };

    let path_clone = socket_path.clone();
    let task = tokio::spawn(async move {
        run_listener(listener, state).await;
    });

    Some(GuardSocketHandle {
        socket_path: path_clone,
        _task: task,
    })
}

/// No Unix domain sockets here: the guard has nothing to listen on.
#[cfg(not(unix))]
pub fn start_listener(project_dir: &Path, _state: SharedGuardState) -> Option<GuardSocketHandle> {
    tracing::debug!(
        "workflow guard socket {} unavailable on this platform",
        guard_socket_path(project_dir).display()
    );
    None
}

/// Run the socket listener loop.
#[cfg(unix)]
async fn run_listener(listener: tokio::net::UnixListener, state: SharedGuardState) {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    loop {
        let (stream, _) = match listener.accept().await {
            Ok(conn) => conn,
            Err(e) => {
                tracing::debug!("workflow guard socket accept error: {e}");
                continue;
            }
        };

        let state = state.clone();
        tokio::spawn(async move {
            let (reader, mut writer) = stream.into_split();
            let mut reader = BufReader::new(reader);
            let mut line = String::new();

            if reader.read_line(&mut line).await.is_err() {
                return;
            }

            let response = handle_query(line.trim(), &state);
            let json = serde_json::to_string(&response)
                .unwrap_or_else(|_| r#"{"allowed":true}"#.to_string());

            let _ = writer.write_all(json.as_bytes()).await;
            let _ = writer.write_all(b"\n").await;
            let _ = writer.shutdown().await;
        });
    }
}

/// Handle a single query line from the MCP server.
#[cfg(any(unix, test))]
fn handle_query(line: &str, state: &SharedGuardState) -> GuardResult {
    let Ok(query) = serde_json::from_str::<serde_json::Value>(line) else {
        return GuardResult {
            allowed: true,
            reason: None,
        };
    };

    let query_type = query.get("query").and_then(|v| v.as_str()).unwrap_or("");

    match query_type {
        "can_transition" => {
            let to_phase = query.get("to").and_then(|v| v.as_str()).unwrap_or("");
            state.can_transition(to_phase)
        }
        _ => GuardResult {
            allowed: true,
            reason: None,
        },
    }
}

/// Handle that cleans up the socket file on drop.
pub struct GuardSocketHandle {
    socket_path: PathBuf,
    _task: tokio::task::JoinHandle<()>,
}

impl Drop for GuardSocketHandle {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

/// Query the workflow guard socket from the MCP server process.
///
/// Connects to the socket, sends the query, and reads the response.
/// Returns `None` if the socket is unavailable (no TUI running = non-workflow
/// mode), which on a platform without Unix domain sockets is always.
#[cfg(not(unix))]
pub fn query_guard(_project_dir: &Path, _to_phase: &str) -> Option<GuardResult> {
    None
}

/// Query the workflow guard socket from the MCP server process.
///
/// Connects to the socket, sends the query, and reads the response.
/// Returns `None` if the socket is unavailable (no TUI running = non-workflow mode).
#[cfg(unix)]
pub fn query_guard(project_dir: &Path, to_phase: &str) -> Option<GuardResult> {
    let socket_path = guard_socket_path(project_dir);

    // Use blocking I/O — MCP tool handlers run in sync context
    let stream = std::os::unix::net::UnixStream::connect(&socket_path).ok()?;
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(2)))
        .ok()?;
    stream
        .set_write_timeout(Some(std::time::Duration::from_secs(2)))
        .ok()?;

    use std::io::{BufRead, Write};

    let query = serde_json::json!({
        "query": "can_transition",
        "to": to_phase,
    });

    let mut stream_ref = &stream;
    writeln!(stream_ref, "{}", serde_json::to_string(&query).ok()?).ok()?;
    stream_ref.flush().ok()?;

    // Shutdown write half to signal end of request
    stream.shutdown(std::net::Shutdown::Write).ok()?;

    let mut reader = std::io::BufReader::new(&stream);
    let mut response_line = String::new();
    reader.read_line(&mut response_line).ok()?;

    serde_json::from_str(&response_line).ok()
}

#[cfg(test)]
#[path = "workflow_socket.tests.rs"]
mod tests;
