//! `magistr mcp` as a real subprocess over its own stdio: the
//! shipping artifact driven by an rmcp client — initialize, tools/list,
//! status, search and an applied edit — and the retained process-level
//! refusals (64 usage, 66 missing root, 75 an unrecognized owner record).
#[path = "support/magistr_command.rs"]
mod magistr_command;

#[path = "support/unified.rs"]
mod unified;

use std::{path::Path, process::Stdio, time::Duration};

use magistr_command::magistr_command;
use rmcp::{
    ServiceExt,
    model::{CallToolRequestParams, RequestMetaObject},
    service::{RoleClient, RunningService},
};
use serde_json::{Value, json};

/// The served binary as a tokio child: its stdin and stdout are the client's
/// transport, exactly as an editor or agent would wire it.
async fn served_client(root: &Path) -> (tokio::process::Child, RunningService<RoleClient, ()>) {
    let mut child = tokio::process::Command::from(served(root))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn the served binary");
    let stdin = child.stdin.take().expect("child stdin");
    let stdout = child.stdout.take().expect("child stdout");
    let client = ().serve((stdout, stdin)).await.expect("initialize");
    (child, client)
}

fn workspace() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("workspace");
    std::fs::create_dir_all(dir.path().join(".git")).expect("repository marker");
    std::fs::create_dir_all(dir.path().join("src")).expect("src");
    std::fs::write(
        dir.path().join("src/app.ts"),
        "export function app(): number {\n  return 1;\n}\n",
    )
    .expect("source");
    dir
}

fn served(root: &Path) -> std::process::Command {
    let mut command = magistr_command();
    command
        .arg("mcp")
        .arg("--dir")
        .arg(root)
        .env("MAGISTR_WORKSPACE_NETWORK_FS", "0");
    command
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_binary_serves_the_workspace_over_its_own_stdio() {
    let dir = workspace();
    let (mut child, client) = served_client(dir.path()).await;
    let info = client.peer().peer_info().expect("server info");
    assert_eq!(
        info.server_info.as_ref().map(|server| server.name.as_str()),
        Some("magistr")
    );
    assert!(info.capabilities.supports_tasks());

    let tools = client
        .peer()
        .list_tools(None)
        .await
        .expect("tools/list")
        .tools;
    let names: Vec<&str> = tools.iter().map(|tool| tool.name.as_ref()).collect();
    for expected in [
        "context", "discover", "read", "search", "edit", "write", "execute",
    ] {
        assert!(names.contains(&expected), "{names:?} lacks {expected}");
    }

    let call = |name: &'static str, args: Value| {
        let peer = client.peer().clone();
        async move {
            let (name, args, composed) = unified::request(name, args);
            let mut request = CallToolRequestParams::new(name);
            request.arguments = args.as_object().cloned();
            let result = peer.call_tool(request).await.expect("tool call");
            unified::value(
                result.structured_content.expect("structured envelope"),
                composed,
            )
        }
    };
    let status = call("workspace_status", json!({})).await;
    assert_eq!(status["ok"], true, "{status}");
    assert_eq!(status["results"][0]["journalMode"], "WAL");
    assert_eq!(
        status["results"][0]["root"].as_str().map(Path::new),
        Some(dir.path().canonicalize().expect("root").as_path())
    );
    let search = call("search", json!({"query": "return", "mode": "lexical"})).await;
    assert_eq!(search["ok"], true, "{search}");
    assert_eq!(search["results"][0]["file"], "src/app.ts");
    let edited = call(
        "edit",
        json!({"op": "replace_text", "file": "src/app.ts", "old": "return 1;", "new": "return 3;"}),
    )
    .await;
    assert_eq!(edited["ok"], true, "{edited}");
    assert!(
        std::fs::read_to_string(dir.path().join("src/app.ts"))
            .expect("edited")
            .contains("return 3;")
    );
    let listed = call("undo", json!({"action": "list"})).await;
    assert_eq!(listed["results"][0]["tool"], "edit");
    // Closing the client closes the child's stdin: the server sees EOF,
    // releases the workspace and exits cleanly.
    client.cancel().await.expect("client closes");
    let status = tokio::time::timeout(Duration::from_secs(30), child.wait())
        .await
        .expect("the server exits when its client leaves")
        .expect("wait");
    assert!(status.success(), "{status}");
}

#[test]
fn missing_roots_and_malformed_configuration_are_refused() {
    let dir = tempfile::tempdir().expect("scratch");
    let missing = served(&dir.path().join("absent")).output().expect("run");
    assert!(!missing.status.success());
    assert!(
        String::from_utf8_lossy(&missing.stderr).contains("No such file"),
        "{}",
        String::from_utf8_lossy(&missing.stderr)
    );
    let mut usage = served(dir.path());
    usage.arg("--config").arg("garbage");
    let usage = usage.output().expect("run");
    assert_eq!(usage.status.code(), Some(64));
    assert!(String::from_utf8_lossy(&usage.stderr).contains("--config expects key=value"));
}

/// Reusing the explicit identity after a crash isolates owner-generation
/// fencing from the fresh fallback identity each MCP connection receives.
async fn session_call(
    client: &RunningService<RoleClient, ()>,
    name: &'static str,
    args: Value,
) -> Value {
    let (name, args, composed) = unified::request(name, args);
    let mut request = CallToolRequestParams::new(name);
    request.arguments = args.as_object().cloned();
    let mut meta = RequestMetaObject::new();
    meta.insert("magistr/sessionId".into(), json!("process-crash-session"));
    request.meta = Some(meta);
    let body = tokio::time::timeout(Duration::from_secs(30), client.peer().call_tool(request))
        .await
        .expect("bounded tool response")
        .expect("tool call")
        .structured_content
        .expect("structured envelope");
    unified::value(body, composed)
}

fn recorded_generation(root: &Path) -> u64 {
    let bytes = std::fs::read(root.join(".magistr/workspace/server.generation"))
        .expect("published native generation");
    assert_eq!(bytes.len(), 20, "complete fixed-width generation");
    String::from_utf8(bytes)
        .expect("generation digits")
        .parse()
        .expect("numeric generation")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn owner_process_crash_releases_admission_and_fences_its_confirmation_on_restart() {
    let dir = tempfile::tempdir().expect("scratch workspace");
    std::fs::create_dir(dir.path().join(".git")).expect("repository marker");
    std::fs::write(dir.path().join("pending.txt"), "reviewed bytes\n").expect("source");
    std::fs::create_dir(dir.path().join(".magistr")).expect("settings directory");
    std::fs::write(
        dir.path().join(".magistr/workspace.toml"),
        "[edit]\nconfirm_mode = \"elicit\"\n",
    )
    .expect("human confirmation mode");
    let (mut first_child, first) =
        tokio::time::timeout(Duration::from_secs(30), served_client(dir.path()))
            .await
            .expect("first owner initializes");
    let request = json!({"op": "move", "from": "pending.txt", "to": "moved.txt"});
    let preview = session_call(&first, "files", request.clone()).await;
    assert_eq!(preview["error"]["code"], "CONFIRM_REQUIRED", "{preview}");
    let confirmation = preview["error"]["confirmToken"]
        .as_str()
        .expect("token")
        .to_owned();
    let generation = recorded_generation(dir.path());
    let state = dir.path().join(".magistr/workspace");
    let marker = std::fs::read(state.join("server.lock")).expect("lock marker");
    let key = std::fs::read(state.join("keys/workspace.key")).expect("workspace key");
    let before = session_call(&first, "undo", json!({"action": "list"})).await;
    assert_eq!(before["results"], json!([]));

    // This handle belongs only to the disposable server spawned above. No
    // lock record supplies a pid, and no active development owner is signalled.
    first_child
        .start_kill()
        .expect("crash the owned scratch child");
    let crashed = tokio::time::timeout(Duration::from_secs(30), first_child.wait())
        .await
        .expect("crashed child exits")
        .expect("reap crashed child");
    assert!(!crashed.success(), "the owner did not shut down gracefully");
    let _ = tokio::time::timeout(Duration::from_secs(5), first.cancel()).await;

    let (mut replacement_child, replacement) =
        tokio::time::timeout(Duration::from_secs(30), served_client(dir.path()))
            .await
            .expect("replacement acquires the released OS lock and initializes");
    assert_eq!(recorded_generation(dir.path()), generation + 1);
    assert_eq!(
        std::fs::read(state.join("server.lock")).expect("same marker"),
        marker
    );
    assert_eq!(
        std::fs::read(state.join("keys/workspace.key")).expect("same key"),
        key
    );
    let mut confirmed = request.clone();
    confirmed["confirm"] = json!(confirmation);
    let refused = session_call(&replacement, "files", confirmed).await;
    assert_eq!(refused["error"]["code"], "CONFIRM_MISMATCH", "{refused}");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("pending.txt")).expect("unchanged source"),
        "reviewed bytes\n"
    );
    assert!(!dir.path().join("moved.txt").exists());
    let after = session_call(&replacement, "undo", json!({"action": "list"})).await;
    assert_eq!(
        after["results"], before["results"],
        "stale approval writes no journal row"
    );

    let fresh = session_call(&replacement, "files", request.clone()).await;
    assert_eq!(fresh["error"]["code"], "CONFIRM_REQUIRED", "{fresh}");
    assert_ne!(fresh["error"]["confirmToken"], json!(confirmation));
    let mut confirmed = request;
    confirmed["confirm"] = fresh["error"]["confirmToken"].clone();
    let applied = session_call(&replacement, "files", confirmed).await;
    assert_eq!(applied["ok"], true, "{applied}");
    assert!(!dir.path().join("pending.txt").exists());
    assert_eq!(
        std::fs::read_to_string(dir.path().join("moved.txt")).expect("approved bytes"),
        "reviewed bytes\n"
    );
    replacement
        .cancel()
        .await
        .expect("replacement client closes");
    let status = tokio::time::timeout(Duration::from_secs(30), replacement_child.wait())
        .await
        .expect("replacement shuts down")
        .expect("reap replacement");
    assert!(status.success(), "{status}");
}

#[test]
fn unknown_lock_refusal_preserves_the_record_and_never_opens_workspace_stores() {
    let dir = workspace();
    let state = dir.path().join(".magistr/workspace");
    std::fs::create_dir_all(&state).expect("state");
    let bytes = "unknown owner record";
    std::fs::write(state.join("server.lock"), bytes).expect("unknown record");
    // Refused admission must leave source and owner state untouched.
    let result = served(dir.path())
        .stdin(Stdio::null())
        .output()
        .expect("native contender");
    assert_eq!(result.status.code(), Some(0), "{result:?}");
    assert!(result.stdout.is_empty(), "no MCP frame without a request");
    let diagnostic = String::from_utf8_lossy(&result.stderr);
    assert!(diagnostic.contains("unrecognized"), "{diagnostic}");
    assert!(diagnostic.contains("serving discovery"), "{diagnostic}");
    assert_eq!(
        std::fs::read_to_string(state.join("server.lock")).expect("preserved"),
        bytes
    );
    for forbidden in ["journal.db", "index.db", "keys"] {
        assert!(
            !state.join(forbidden).exists(),
            "refusal opened {forbidden}"
        );
    }
}
