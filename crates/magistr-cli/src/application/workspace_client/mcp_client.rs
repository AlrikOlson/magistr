//! One MCP server on the other end of a pipe.

use std::io::{BufRead as _, BufReader, Write as _};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

/// The protocol revision the cockpit asks for. A server that speaks a
/// later one answers with its own; nothing here depends on the value
/// beyond the handshake, so a newer server is not turned away.
const PROTOCOL: &str = "2025-06-18";

/// How long the handshake gets before the server is called unreachable.
/// Generous: a cold `bun run` of a TypeScript server indexes on start.
const HANDSHAKE: Duration = Duration::from_secs(20);

/// Lines of the server's stderr kept as the reason a call failed — a
/// server that refuses to start says why there and then exits, so its
/// last words are the only honest thing to put on the stage.
const STDERR_KEPT: usize = 8;

/// One MCP server the cockpit drives over its stdio: newline-delimited
/// JSON-RPC out on the child's stdin, its answers in on the child's
/// stdout. Blocking by construction — [`WorkbenchPort`] is synchronous
/// — but never unboundedly so: a reader thread owns the pipe and every
/// answer is waited for with a deadline, so a server that wedges costs
/// one call, not the cockpit.
///
/// Dropping the client kills the child.
///
/// [`WorkbenchPort`]: super::WorkbenchPort
pub struct McpClient {
    child: Child,
    stdin: ChildStdin,
    /// The reader thread's line channel. Behind a mutex only to make the
    /// client `Sync`, which [`WorkbenchPort`](super::WorkbenchPort)
    /// requires; every read goes through `&mut self`.
    lines: Mutex<Receiver<String>>,
    /// The server's last words, for the reason a call could not be made.
    stderr: Arc<Mutex<Vec<String>>>,
    next_id: u64,
}

impl McpClient {
    /// Spawn `command args` with `env` over its environment, in `dir`
    /// when one is given, complete the MCP handshake, and hand back the
    /// connected client.
    ///
    /// `dir` is the project root, and it is load-bearing rather than a
    /// convenience: a `.mcp.json` launch line is written with relative
    /// paths (`bun run src/main.ts .`) so that a committed file carries
    /// no path from the machine that wrote it, and such a line only
    /// resolves from the project's own directory. Without it the server
    /// cannot start at all wherever the cockpit was launched from
    /// somewhere else.
    ///
    /// # Errors
    /// The server's own words when it does not start, does not answer
    /// the handshake in time, or refuses it.
    pub fn connect(
        command: &str,
        args: &[String],
        env: &[(String, String)],
        dir: Option<&std::path::Path>,
    ) -> Result<Self, String> {
        let mut spawning = Command::new(command);
        spawning
            .args(args)
            .envs(env.iter().map(|(key, value)| (key, value)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(dir) = dir {
            spawning.current_dir(dir);
        }
        let mut child = spawning
            .spawn()
            .map_err(|err| format!("{command} did not start: {err}"))?;
        let (Some(stdin), Some(stdout), Some(stderr)) =
            (child.stdin.take(), child.stdout.take(), child.stderr.take())
        else {
            let _ = child.kill();
            return Err(format!("{command} was started without its pipes"));
        };
        let (tx, lines) = channel();
        thread::spawn(move || pump(stdout, &tx));
        let kept = Arc::new(Mutex::new(Vec::new()));
        let keeping = Arc::clone(&kept);
        thread::spawn(move || keep(stderr, &keeping));
        let mut client = Self {
            child,
            stdin,
            lines: Mutex::new(lines),
            stderr: kept,
            next_id: 1,
        };
        client.handshake()?;
        Ok(client)
    }

    /// Call `tool` with `arguments` and hand back the response envelope
    /// (`{v, ok, results, notes, …}`) the workbench answered with.
    ///
    /// # Errors
    /// The transport's reason the call did not complete, or — for an
    /// envelope that came back `ok: false` — the workbench's own words.
    pub fn call(
        &mut self,
        tool: &str,
        arguments: Value,
        timeout: Duration,
    ) -> Result<Value, String> {
        let advanced = !matches!(tool, "search" | "read" | "edit" | "write");
        let (name, arguments) = if advanced {
            let source = format!(
                "return magistr.call({}, {});",
                serde_json::to_string(tool).map_err(|e| e.to_string())?,
                arguments
            );
            ("execute", json!({"source":source,"maxOperations":1}))
        } else {
            (tool, arguments)
        };
        let id = self.request(
            "tools/call",
            json!({
                "name": name,
                "arguments": arguments,
                "_meta": { "workbench/sessionId": "magistr-cockpit" },
            }),
        )?;
        let result = self.answer(id, timeout)?;
        let result = envelope(&result)?;
        if advanced {
            Ok(result["value"].clone())
        } else {
            Ok(result)
        }
    }

    /// `initialize`, then the `initialized` notification — the order the
    /// protocol requires before any tool may be called.
    fn handshake(&mut self) -> Result<(), String> {
        let id = self.request(
            "initialize",
            json!({
                "protocolVersion": PROTOCOL,
                "capabilities": {},
                "clientInfo": {
                    "name": "magistr-cockpit",
                    "version": env!("CARGO_PKG_VERSION"),
                },
            }),
        )?;
        self.answer(id, HANDSHAKE)?;
        self.notify("notifications/initialized")
    }

    /// Write one request and hand back the id its answer will carry.
    fn request(&mut self, method: &str, params: Value) -> Result<u64, String> {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.write(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        }))?;
        Ok(id)
    }

    /// Write one notification — no id, so no answer is waited for.
    fn notify(&mut self, method: &str) -> Result<(), String> {
        self.write(&json!({ "jsonrpc": "2.0", "method": method }))
    }

    fn write(&mut self, message: &Value) -> Result<(), String> {
        writeln!(self.stdin, "{message}")
            .and_then(|()| self.stdin.flush())
            .map_err(|err| self.why(&format!("the workbench stopped listening: {err}")))
    }

    /// Wait for the answer carrying `id`, skipping the server's
    /// notifications and any answer to a call already given up on.
    fn answer(&mut self, id: u64, timeout: Duration) -> Result<Value, String> {
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(self.why("the workbench did not answer in time"));
            }
            // The poison case drops its borrow of `self` here so the
            // reason below may read the server's last words.
            let received = self
                .lines
                .get_mut()
                .map_err(|_| ())
                .map(|lines| lines.recv_timeout(left));
            let line = match received {
                Ok(Ok(line)) => line,
                Ok(Err(RecvTimeoutError::Timeout)) => {
                    return Err(self.why("the workbench did not answer in time"));
                }
                Ok(Err(RecvTimeoutError::Disconnected)) | Err(()) => {
                    return Err(self.why("the workbench stopped answering"));
                }
            };
            let Ok(message) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if message["id"].as_u64() != Some(id) {
                continue;
            }
            if let Some(error) = message["error"]["message"].as_str() {
                return Err(error.to_owned());
            }
            return Ok(message["result"].clone());
        }
    }

    /// `what`, with the server's last words when it left any — the
    /// difference between "the workbench stopped answering" and the
    /// reason it stopped.
    fn why(&self, what: &str) -> String {
        let last = self
            .stderr
            .lock()
            .ok()
            .and_then(|kept| kept.last().cloned())
            .unwrap_or_default();
        let last = last.trim().to_owned();
        if last.is_empty() {
            what.to_owned()
        } else {
            format!("{what}: {last}")
        }
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The response envelope inside one `tools/call` result: the structured
/// content when the server sent it, else the text block parsed as the
/// same JSON. An envelope that came back `ok: false` is the workbench
/// refusing, and its message is the error.
fn envelope(result: &Value) -> Result<Value, String> {
    // Check failure before trying to dereference a success payload. Retain the
    // complete result: codes, field problems and uncertain-write evidence matter.
    if result["isError"] == true {
        return Err(format!("workbench tool failed: {result}"));
    }
    let unreadable = || format!("the workbench answered nothing the cockpit could read: {result}");
    let envelope = if result["structuredContent"].is_object() {
        result["structuredContent"].clone()
    } else {
        result["content"]
            .as_array()
            .and_then(|items| {
                items.iter().find_map(|item| {
                    item["text"]
                        .as_str()
                        .and_then(|text| serde_json::from_str::<Value>(text).ok())
                })
            })
            .ok_or_else(unreadable)?
    };
    if envelope["ok"] != true {
        return Err(format!("the workbench refused: {result}"));
    }
    Ok(envelope)
}

/// Feed every line the server writes into the channel until it ends.
fn pump(stdout: std::process::ChildStdout, tx: &Sender<String>) {
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        if tx.send(line).is_err() {
            return;
        }
    }
}

/// Keep the server's last [`STDERR_KEPT`] lines.
fn keep(stderr: std::process::ChildStderr, kept: &Mutex<Vec<String>>) {
    for line in BufReader::new(stderr).lines().map_while(Result::ok) {
        let Ok(mut kept) = kept.lock() else {
            return;
        };
        if kept.len() == STDERR_KEPT {
            kept.remove(0);
        }
        kept.push(line);
    }
}

#[cfg(test)]
#[path = "mcp_client.tests.rs"]
mod tests;
