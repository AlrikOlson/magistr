//! Real-process fixtures for native workspace owner attachment.
#[path = "../support/magistr_command.rs"]
mod magistr_command;
#[path = "../support/unified.rs"]
mod unified;

use rmcp::{
    ServiceExt,
    model::{CallToolRequestParams, RequestMetaObject},
    service::{RoleClient, RunningService},
};
use serde_json::{Value, json};
use std::{path::Path, process::Stdio, time::Duration};

pub(super) const WAIT: Duration = Duration::from_secs(30);

pub(super) struct Connection {
    pub(super) child: tokio::process::Child,
    pub(super) client: RunningService<RoleClient, ()>,
}

impl Connection {
    pub(super) async fn open(root: &Path) -> Self {
        Self::open_with(command(root)).await
    }

    /// A client over any invocation of the binary that serves MCP on stdio.
    pub(super) async fn open_with(mut command: tokio::process::Command) -> Self {
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn the workspace client");
        let stdin = child.stdin.take().expect("child stdin");
        let stdout = child.stdout.take().expect("child stdout");
        let client = tokio::time::timeout(WAIT, ().serve((stdout, stdin)))
            .await
            .expect("bounded initialization")
            .expect("initialize against the native workspace owner");
        Self { child, client }
    }

    pub(super) async fn call(&self, name: &str, args: Value) -> Value {
        self.session_call(name, args, None).await
    }

    pub(super) async fn session_call(
        &self,
        name: &str,
        args: Value,
        session: Option<&str>,
    ) -> Value {
        let (public, args, composed) = unified::request(name, args);
        let mut request = CallToolRequestParams::new(public);
        request.arguments = args.as_object().cloned();
        if let Some(session) = session {
            let mut meta = RequestMetaObject::new();
            meta.insert("magistr/sessionId".into(), json!(session));
            request.meta = Some(meta);
        }
        let result = tokio::time::timeout(WAIT, self.client.peer().call_tool(request))
            .await
            .expect("bounded tool response")
            .expect("tool response");
        let text = result
            .content
            .iter()
            .find_map(|block| block.as_text().map(|text| text.text.as_str()))
            .expect("text envelope");
        let parsed: Value = serde_json::from_str(text).expect("envelope JSON");
        assert_eq!(Some(&parsed), result.structured_content.as_ref());
        unified::value(parsed, composed)
    }

    pub(super) async fn assert_tools(&self) {
        let tools = tokio::time::timeout(WAIT, self.client.peer().list_tools(None))
            .await
            .expect("bounded tools/list")
            .expect("tools/list")
            .tools;
        let mut names: Vec<_> = tools.iter().map(|tool| tool.name.as_ref()).collect();
        names.sort_unstable();
        assert_eq!(
            names,
            [
                "check_work",
                "context",
                "discover",
                "edit",
                "execute",
                "finish_work",
                "plan_work",
                "read",
                "record_progress",
                "search",
                "start_work",
                "write"
            ]
        );
    }

    pub(super) async fn close(self) {
        let Self { mut child, client } = self;
        tokio::time::timeout(WAIT, client.cancel())
            .await
            .expect("bounded client cancellation")
            .expect("client closes");
        let status = tokio::time::timeout(WAIT, child.wait())
            .await
            .expect("client process exits after EOF")
            .expect("reap client process");
        assert!(status.success(), "{status}");
    }

    /// The bridge process is still serving its client.
    pub(super) fn assert_alive(&mut self) {
        assert!(
            self.child
                .try_wait()
                .expect("poll the bridge process")
                .is_none(),
            "the bridge exited"
        );
    }
}

/// Make native admission fail closed without touching any live owner: the
/// scratch root's marker is rewritten to an unknown record the native lock
/// refuses. Returns the native bytes so the caller can restore them.
pub(super) fn deny_admission(root: &Path) -> Vec<u8> {
    let lock = root.join(".magistr/workspace/server.lock");
    let native = std::fs::read(&lock).expect("native marker");
    std::fs::write(&lock, b"{\"pid\":0,\"host\":\"unrecognized-fixture\"}\n")
        .expect("unrecognized marker");
    native
}

pub(super) fn restore_admission(root: &Path, native: &[u8]) {
    std::fs::write(root.join(".magistr/workspace/server.lock"), native)
        .expect("native marker restored");
}

pub(super) fn command(root: &Path) -> tokio::process::Command {
    let mut command = magistr();
    command
        .args(["mcp", "--dir"])
        .arg(root)
        .env("MAGISTR_WORKFLOW_DATA_DIR", root.join("workflow-state"));
    command
}

/// The binary under test with the workspace environment every fixture uses.
pub(super) fn magistr() -> tokio::process::Command {
    let mut command = magistr_command::magistr_command();
    command
        .env("MAGISTR_WORKSPACE_NETWORK_FS", "0")
        .env_remove("MAGISTR_WORKSPACE_ORG_POLICY")
        .env_remove("MAGISTR_WORKSPACE_USER_CONFIG");
    tokio::process::Command::from(command)
}

/// The path of the binary under test, for launch lines written to disk.
pub(super) fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_magistr")
}

pub(super) fn workspace() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("workspace");
    std::fs::create_dir(dir.path().join(".git")).expect("repository marker");
    std::fs::create_dir(dir.path().join(".magistr")).expect("settings directory");
    // Exercise a live watcher and its joined shutdown without depending on
    // macOS FSEvents daemon registration under parallel process load. Native
    // watcher behavior is covered by workspace_index and index_watcher tests.
    std::fs::write(
        dir.path().join(".magistr/workspace.toml"),
        "[watcher]\nmode = 'poll'\n",
    )
    .expect("deterministic lifecycle watcher");
    for file in ["alice.txt", "bob.txt", "shared.txt", "pending.txt"] {
        std::fs::write(dir.path().join(file), "original bytes\n").expect("source fixture");
    }
    dir
}

pub(super) fn generation(root: &Path) -> u64 {
    let bytes = std::fs::read(root.join(".magistr/workspace/server.generation"))
        .expect("published generation");
    assert_eq!(bytes.len(), 20, "complete fixed-width generation");
    String::from_utf8(bytes)
        .expect("generation digits")
        .parse()
        .expect("numeric generation")
}

pub(super) fn edit(file: &str, replacement: &str) -> Value {
    json!({"op": "replace_text", "file": file, "old": "original bytes", "new": replacement})
}

pub(super) fn file_text(root: &Path, file: &str) -> String {
    std::fs::read_to_string(root.join(file)).expect("workspace bytes")
}

pub(super) fn read_text(envelope: &Value) -> String {
    envelope["results"]
        .as_array()
        .expect("read rows")
        .iter()
        .filter_map(|row| row["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn assert_ok(envelope: &Value) {
    assert_eq!(envelope["ok"], true, "{envelope}");
}
