//! `magistr workspace serve` as a real subprocess over its own stdio: the
//! shipping artifact driven by an rmcp client — initialize, tools/list,
//! status, search and an applied edit — and the retained process-level
//! refusals (64 usage, 66 missing root, 75 a live owner).
#[path = "support/magistr_command.rs"]
mod magistr_command;

use std::{
    path::Path,
    process::Stdio,
    time::{Duration, Instant},
};

use magistr_command::magistr_command;
use rmcp::{
    ServiceExt,
    model::CallToolRequestParams,
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
        .arg("workspace")
        .arg("serve")
        .arg(root)
        .env("WORKBENCH_NETWORK_FS", "0");
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
        "workbench_status",
        "map",
        "read",
        "search",
        "edit",
        "files",
        "undo",
        "script",
        "run",
    ] {
        assert!(names.contains(&expected), "{names:?} lacks {expected}");
    }

    let call = |name: &'static str, args: Value| {
        let peer = client.peer().clone();
        async move {
            let mut request = CallToolRequestParams::new(name);
            request.arguments = args.as_object().cloned();
            let result = peer.call_tool(request).await.expect("tool call");
            result.structured_content.expect("structured envelope")
        }
    };
    let status = call("workbench_status", json!({})).await;
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
fn a_missing_root_and_a_malformed_config_are_refused_with_the_retained_exit_codes() {
    let dir = tempfile::tempdir().expect("scratch");
    let missing = served(&dir.path().join("absent")).output().expect("run");
    assert_eq!(missing.status.code(), Some(66));
    assert!(
        String::from_utf8_lossy(&missing.stderr).contains("workspace root does not exist"),
        "{}",
        String::from_utf8_lossy(&missing.stderr)
    );
    let mut usage = served(dir.path());
    usage.arg("--config").arg("garbage");
    let usage = usage.output().expect("run");
    assert_eq!(usage.status.code(), Some(64));
    assert!(String::from_utf8_lossy(&usage.stderr).contains("--config expects key=value"));
}

#[test]
fn a_second_instance_is_refused_with_exit_75_while_the_first_owns_the_workspace() {
    let dir = workspace();
    let mut first = served(dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("first owner");
    let lock = dir.path().join(".workbench/server.lock");
    let deadline = Instant::now() + Duration::from_secs(30);
    while !lock.exists() {
        assert!(
            Instant::now() < deadline,
            "the first owner never took the lock"
        );
        assert!(
            first.try_wait().expect("poll").is_none(),
            "the first owner exited early"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    // The lock file exists a moment before the flock lands; a live owner
    // either refuses us at once or within the next few retries.
    let mut code = None;
    for _ in 0..50 {
        let second = served(dir.path())
            .stdin(Stdio::null())
            .output()
            .expect("second instance");
        code = second.status.code();
        if code == Some(75) {
            let stderr = String::from_utf8_lossy(&second.stderr);
            assert!(stderr.contains("already owns this workspace"), "{stderr}");
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(code, Some(75));
    drop(first.stdin.take());
    let status = first
        .wait()
        .expect("the first owner exits when stdin closes");
    assert!(status.success(), "{status}");
    assert!(
        lock.is_file(),
        "the lock file stays; the OS lock is what released"
    );
}
