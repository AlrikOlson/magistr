//! The workbench port over a real workbench-mcp server.

use std::time::Duration;

use serde_json::{Value, json};

use super::journal_entry::JournalEntry;
use super::mcp_client::McpClient;
use super::port::WorkbenchPort;
use super::run_row::RunRow;

/// How long a read, a search or a map gets.
const CALL: Duration = Duration::from_secs(30);

/// How long a run gets: the server's own default wall-clock limit is
/// five minutes, so the cockpit waits a little longer than that and
/// lets the server be the one to give up.
const RUN: Duration = Duration::from_secs(310);

/// Tokens a whole-file read may spend — a file the viewer pane opens
/// should arrive whole, not head-and-tail.
const READ_BUDGET: u64 = 200_000;

/// Tokens a search or a map may spend.
const LIST_BUDGET: u64 = 8_000;

/// How many journal rows one refresh asks the CLI for — plenty for the
/// Changes tab, bounded so a long-lived workspace's journal does not
/// arrive whole.
const JOURNAL_LIMIT: &str = "200";

/// A [`WorkbenchPort`] speaking MCP over stdio to the configured
/// workbench-mcp server, with the write journal read through the same
/// binary's `journal` CLI subcommand.
///
/// The server is spawned lazily, on the first call that needs it, and
/// at most once. The native workspace bridge attaches to the agent's own
/// owner during `magistr run`, so the cockpit and the agent act through one
/// admitted writer; the retained legacy server still takes an exclusive
/// lock and refuses. A refusal is remembered and answered — the server's
/// own words land on the stage — rather than retried, which would spawn a
/// doomed child per click.
///
/// [`journal`](WorkbenchPort::journal) deliberately does NOT go through
/// the server: the CLI takes no lock, so it answers *during* a run —
/// which is when the Changes tab most wants it. [`undo`](WorkbenchPort::undo)
/// does: through the native owner it lands as a workspace-scoped rewind of
/// one named op with the server's conflict surfacing; under the legacy
/// server the exclusive lock's refusal is the enforcement.
pub struct McpWorkbench {
    command: String,
    args: Vec<String>,
    env: Vec<(String, String)>,
    /// The directory the journal CLI runs in, so a portable "." root in
    /// the launch line resolves against the project rather than
    /// wherever the cockpit process happens to sit.
    workspace: Option<std::path::PathBuf>,
    client: Option<McpClient>,
    /// The reason the server could not be connected to, once one was
    /// given: every later call answers it without spawning again. The
    /// CLI path never reads or writes this — it is about the transport.
    refused: Option<String>,
}

impl McpWorkbench {
    /// A port that will run `command args` with `env` over its
    /// environment when it is first asked for something.
    #[must_use]
    pub fn new(command: impl Into<String>, args: Vec<String>, env: Vec<(String, String)>) -> Self {
        Self {
            command: command.into(),
            args,
            env,
            workspace: None,
            client: None,
            refused: None,
        }
    }

    /// The port with the journal CLI's working directory pinned to the
    /// project.
    #[must_use]
    pub fn with_workspace(mut self, dir: impl Into<std::path::PathBuf>) -> Self {
        self.workspace = Some(dir.into());
        self
    }

    /// The `journal` CLI's argument list, derived from the server's own
    /// launch line: the trailing argument is the workspace root and
    /// everything before it is the launcher prefix, so
    /// `bun run src/main.ts .` becomes
    /// `bun run src/main.ts journal . --json --limit 200` and the CLI
    /// can never point at a different workbench than the server. Magistr's
    /// own launcher, `magistr mcp-launch workbench-mcp .`, names the native
    /// workspace bridge; its journal verb is `magistr workspace journal`.
    fn journal_args(&self) -> Vec<String> {
        let (leading, root) = match self.args.split_last() {
            Some((root, leading)) => (leading, root.as_str()),
            None => (&[] as &[String], "."),
        };
        let mut args: Vec<String> = match leading {
            [verb, target] if verb == "mcp-launch" && target == "workbench-mcp" => {
                vec!["workspace".to_owned()]
            }
            leading => leading.to_vec(),
        };
        args.extend(
            ["journal", root, "--json", "--limit", JOURNAL_LIMIT]
                .into_iter()
                .map(str::to_owned),
        );
        args
    }

    /// The connected server, connecting on the first call.
    fn client(&mut self) -> Result<&mut McpClient, String> {
        if let Some(refused) = &self.refused {
            return Err(refused.clone());
        }
        if self.client.is_none() {
            match McpClient::connect(
                &self.command,
                &self.args,
                &self.env,
                self.workspace.as_deref(),
            ) {
                Ok(client) => self.client = Some(client),
                Err(err) => {
                    self.refused = Some(err.clone());
                    return Err(err);
                }
            }
        }
        self.client
            .as_mut()
            .ok_or_else(|| "the workbench was not connected".to_owned())
    }
}

impl WorkbenchPort for McpWorkbench {
    fn read(&mut self, path: &str) -> Result<String, String> {
        let envelope =
            self.client()?
                .call("read", json!({ "file": path, "budget": READ_BUDGET }), CALL)?;
        Ok(content(&envelope))
    }

    fn undo(&mut self, id: &str) -> Result<(), String> {
        let seq: u64 = id
            .parse()
            .map_err(|_| format!("the journal names edits by op seq — '{id}' is not one"))?;
        let client = self.client().map_err(|err| {
            format!(
                "the workspace could not be opened to rewind: {err} — if an agent \
                 session is running, its server owns this workspace; ask the agent to undo"
            )
        })?;
        let envelope = client.call("undo", json!({ "ops": [seq], "scope": "workspace" }), CALL)?;
        undone(&envelope, seq)
    }

    fn journal(&mut self) -> Result<Vec<JournalEntry>, String> {
        let mut command = std::process::Command::new(&self.command);
        command
            .args(self.journal_args())
            .envs(self.env.iter().map(|(key, value)| (key, value)))
            .stdin(std::process::Stdio::null());
        if let Some(dir) = &self.workspace {
            command.current_dir(dir);
        }
        let output = output_of(command, &self.command)?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        let said = stderr.lines().last().unwrap_or_default().trim();
        match output.status.code() {
            Some(0) => journal_entries(&String::from_utf8_lossy(&output.stdout)),
            // The CLI's own taxonomy: 65 is durable data this binary
            // cannot read, 66 is no such workspace. Words, not codes.
            Some(65) => Err(format!(
                "the journal here was written by a newer workbench-mcp than the one \
                 magistr launches — upgrade it ({said})"
            )),
            Some(66) => Err(format!("no workbench journal to list — {said}")),
            Some(code) => Err(format!("the journal listing failed (exit {code}): {said}")),
            None => Err("the journal listing was killed before it answered".to_owned()),
        }
    }

    fn search(&mut self, query: &str) -> Result<Vec<String>, String> {
        let envelope = self.client()?.call(
            "search",
            json!({ "query": query, "budget": LIST_BUDGET }),
            CALL,
        )?;
        Ok(hits(&envelope))
    }

    fn map(&mut self) -> Result<Vec<String>, String> {
        let envelope = self.client()?.call(
            "map",
            json!({ "detail": "skeleton", "budget": LIST_BUDGET }),
            CALL,
        )?;
        Ok(entries(&envelope))
    }

    fn run(&mut self, command: &str) -> Result<Vec<RunRow>, String> {
        let envelope = self
            .client()?
            .call("run", json!({ "command": command }), RUN)?;
        Ok(rows(&envelope, command))
    }
}

/// The envelope's `results` array, or nothing.
fn results(envelope: &Value) -> &[Value] {
    envelope["results"].as_array().map_or(&[], Vec::as_slice)
}

/// Run a prepared command to completion, with its own words when it
/// cannot start.
fn output_of(
    mut command: std::process::Command,
    name: &str,
) -> Result<std::process::Output, String> {
    command
        .output()
        .map_err(|err| format!("{name} did not start: {err}"))
}

/// The CLI's journal envelope as the port's entries — one per `op` row,
/// reversed, because the CLI answers newest first and the port's
/// contract is oldest first.
fn journal_entries(stdout: &str) -> Result<Vec<JournalEntry>, String> {
    let envelope: Value = serde_json::from_str(stdout.trim())
        .map_err(|err| format!("the journal listing was not the CLI's JSON: {err}"))?;
    let mut entries: Vec<JournalEntry> = results(&envelope)
        .iter()
        .filter(|result| result["kind"] == json!("op"))
        .map(|op| JournalEntry {
            id: op["seq"].as_u64().unwrap_or(0).to_string(),
            path: op["path"].as_str().unwrap_or_default().to_owned(),
            added: u32::try_from(op["added"].as_u64().unwrap_or(0)).unwrap_or(u32::MAX),
            removed: u32::try_from(op["removed"].as_u64().unwrap_or(0)).unwrap_or(u32::MAX),
            parse_ok: op["parseOk"].as_bool().unwrap_or(false),
            diff: hunk_body(op["diff"].as_str().unwrap_or_default()),
        })
        .collect();
    entries.reverse();
    Ok(entries)
}

/// The unified diff without its `--- a/…` / `+++ b/…` file headers —
/// [`JournalEntry::diff`]'s documented shape is the `@@` hunks and
/// their lines, which is what `turn::diff_kind` classifies.
fn hunk_body(diff: &str) -> String {
    diff.lines()
        .skip_while(|line| line.starts_with("--- ") || line.starts_with("+++ "))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Whether the server's `undo` answer actually rewound `seq`. A
/// `conflict` row means the file moved on and nothing was forced; a
/// note naming the op as not undoable means it was skipped — already
/// undone, an undo's own op, or another session's. Both are refusals
/// carrying the server's own words.
fn undone(envelope: &Value, seq: u64) -> Result<(), String> {
    for result in results(envelope) {
        if result["kind"] == json!("conflict") {
            let path = result["path"].as_str().unwrap_or("a file");
            let reason = result["reason"].as_str().unwrap_or("it moved on");
            return Err(format!("op {seq} was not rewound — {path}: {reason}"));
        }
    }
    if let Some(notes) = envelope["notes"].as_array() {
        for note in notes {
            if let Some(note) = note.as_str()
                && note.contains("not undoable")
            {
                return Err(note.to_owned());
            }
        }
    }
    Ok(())
}

/// A read's segments joined back into the file: `read` answers a unit
/// header and then one `lines` segment per stretch it returned.
fn content(envelope: &Value) -> String {
    let mut text: Vec<&str> = Vec::new();
    for result in results(envelope) {
        if result["kind"] == json!("lines")
            && let Some(body) = result["text"].as_str()
        {
            text.push(body);
        }
    }
    text.join("\n")
}

/// A search's hits as one line each — the shape the viewer pane shows
/// and the fake answers with: `path:line: the matching line`.
fn hits(envelope: &Value) -> Vec<String> {
    let mut lines = Vec::new();
    for result in results(envelope) {
        let Some(file) = result["file"].as_str() else {
            continue;
        };
        let Some(hits) = result["hits"].as_array() else {
            continue;
        };
        for hit in hits {
            let line = hit["line"].as_u64().unwrap_or(0);
            let excerpt = hit["excerpt"].as_str().unwrap_or_default().trim();
            lines.push(format!("{file}:{line}: {excerpt}"));
        }
    }
    lines
}

/// A map's tree as one line per entry, directories with a trailing
/// slash. The overview row is the server's own health, not the
/// workspace, so it does not become a line.
fn entries(envelope: &Value) -> Vec<String> {
    let mut lines = Vec::new();
    for result in results(envelope) {
        let Some(path) = result["path"].as_str() else {
            continue;
        };
        match result["kind"].as_str() {
            Some("dir") => lines.push(format!("{path}/")),
            Some("file") => lines.push(path.to_owned()),
            _ => {}
        }
    }
    lines
}

/// A run's rows. The server answers one row per run, shaped by what it
/// could structure: a `report` from a test runner adapter carries the
/// failures, `diagnostics` from a compiler or lint carries the
/// findings, and `output` is the honest fallback. Each becomes its
/// findings followed by the run's own verdict row.
fn rows(envelope: &Value, command: &str) -> Vec<RunRow> {
    let mut rows = Vec::new();
    for result in results(envelope) {
        match result["kind"].as_str() {
            Some("report") => rows.extend(failures(result)),
            Some("diagnostics") => rows.extend(diagnostics(result)),
            _ => {}
        }
        rows.push(verdict(result, command));
    }
    rows
}

/// One row per structured test failure.
fn failures(result: &Value) -> Vec<RunRow> {
    let Some(failures) = result["failures"].as_array() else {
        return Vec::new();
    };
    failures
        .iter()
        .map(|failure| RunRow {
            name: failure["testName"].as_str().unwrap_or("a test").to_owned(),
            passed: false,
            detail: failure["assertion"]
                .as_str()
                .unwrap_or_default()
                .trim()
                .to_owned(),
        })
        .collect()
}

/// One row per compiler or lint diagnostic, named by where it is.
fn diagnostics(result: &Value) -> Vec<RunRow> {
    let Some(diagnostics) = result["diagnostics"].as_array() else {
        return Vec::new();
    };
    diagnostics
        .iter()
        .map(|diagnostic| {
            let where_ = match (diagnostic["file"].as_str(), diagnostic["line"].as_u64()) {
                (Some(file), Some(line)) => format!("{file}:{line}"),
                (Some(file), None) => file.to_owned(),
                _ => diagnostic["level"].as_str().unwrap_or("note").to_owned(),
            };
            let code = diagnostic["code"].as_str().unwrap_or_default();
            let message = diagnostic["message"].as_str().unwrap_or_default();
            let detail = if code.is_empty() {
                message.to_owned()
            } else {
                format!("{code}: {message}")
            };
            RunRow {
                name: where_,
                // A warning is a finding too — the verdict row below is
                // where the run's pass or fail is said.
                passed: false,
                detail,
            }
        })
        .collect()
}

/// The run's own verdict: its summary when the adapter wrote one, else
/// the command, and the exit code as the detail when it was not zero.
fn verdict(result: &Value, command: &str) -> RunRow {
    let exit = result["exitCode"].as_i64();
    let timed_out = result["timedOut"].as_bool().unwrap_or(false);
    let name = result["summary"]
        .as_str()
        .map(str::trim)
        .filter(|summary| !summary.is_empty())
        .unwrap_or(command)
        .to_owned();
    let detail = if timed_out {
        "timed out".to_owned()
    } else {
        match exit {
            Some(0) => String::new(),
            Some(code) => format!("exit {code}"),
            None => "no exit code".to_owned(),
        }
    };
    RunRow {
        name,
        passed: exit == Some(0) && !timed_out,
        detail,
    }
}

#[cfg(test)]
#[path = "mcp_workbench.tests.rs"]
mod tests;
