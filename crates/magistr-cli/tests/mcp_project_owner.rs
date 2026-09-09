//! Real stdio subprocess regressions for cooperative project-owner admission.
#![cfg(unix)]

#[path = "support/magistr_command.rs"]
mod magistr_command;

use std::path::Path;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdout, Command};
use tokio::time::{Duration, timeout};

struct Server {
    child: Child,
    lines: Lines<BufReader<ChildStdout>>,
}

impl Server {
    async fn spawn(root: &Path, data: &Path, socket: &Path) -> Self {
        let mut child = Command::from(magistr_command::magistr_command())
            .arg("mcp-serve")
            .current_dir(root)
            .env("THINK_AND_SHIP_DATA_DIR", data)
            .env("THINK_AND_SHIP_PERSIST", "true")
            .env("THINK_AND_SHIP_BROADCAST_PATH", socket)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn MCP server");
        let lines = BufReader::new(child.stdout.take().expect("stdout")).lines();
        let mut server = Self { child, lines };
        server
            .send(json!({
                "jsonrpc":"2.0", "id":1, "method":"initialize",
                "params":{"protocolVersion":"2025-06-18","capabilities":{},
                    "clientInfo":{"name":"owner-test","version":"1"}}
            }))
            .await;
        server
    }

    async fn send(&mut self, value: Value) {
        let bytes = format!("{value}\n");
        self.child
            .stdin
            .as_mut()
            .expect("stdin")
            .write_all(bytes.as_bytes())
            .await
            .expect("request");
    }

    async fn response(&mut self) -> Option<Value> {
        timeout(Duration::from_secs(15), self.lines.next_line())
            .await
            .expect("bounded response")
            .expect("stdout line")
            .map(|line| serde_json::from_str(&line).expect("MCP JSON"))
    }

    async fn initialized(&mut self) {
        self.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .await;
    }

    async fn call(&mut self, name: &str, arguments: Value) -> Value {
        self.send(json!({"jsonrpc":"2.0","id":2,"method":"tools/call",
            "params":{"name":name,"arguments":arguments}}))
            .await;
        loop {
            let response = self.response().await.expect("tool response");
            if response["id"] == 2 {
                let result = &response["result"];
                assert_ne!(result["isError"], true, "{response}");
                return result.get("structuredContent").cloned().unwrap_or_else(|| {
                    serde_json::from_str(result["content"][0]["text"].as_str().expect("text"))
                        .expect("structured JSON")
                });
            }
        }
    }

    async fn rejection(&mut self) -> String {
        let status = timeout(Duration::from_secs(15), self.child.wait())
            .await
            .expect("bounded rejection")
            .expect("exit status");
        assert!(
            !status.success(),
            "rejected server must exit unsuccessfully"
        );
        let mut stderr = String::new();
        self.child
            .stderr
            .take()
            .expect("stderr")
            .read_to_string(&mut stderr)
            .await
            .expect("stderr text");
        stderr
    }
}

fn identity(root: &Path) {
    std::fs::create_dir_all(root.join(".think-and-ship")).expect("identity directory");
    std::fs::write(
        root.join(".think-and-ship/project.json"),
        "{\"id\":\"owner-test\"}\n",
    )
    .expect("identity");
}

#[tokio::test]
async fn simultaneous_stdio_start_has_one_owner_and_kill_allows_fresh_restart() {
    let tmp = tempfile::tempdir().expect("tempdir");
    identity(tmp.path());
    let data = tmp.path().join("data");
    // Distinct sockets cannot bypass store ownership.
    let mut a = Server::spawn(tmp.path(), &data, &tmp.path().join("a.sock")).await;
    let mut b = Server::spawn(tmp.path(), &data, &tmp.path().join("b.sock")).await;
    let (ra, rb) = tokio::join!(a.response(), b.response());
    assert_ne!(
        ra.is_some(),
        rb.is_some(),
        "exactly one initialized: {ra:?}, {rb:?}"
    );
    let (mut owner, mut rejected) = if ra.is_some() { (a, b) } else { (b, a) };
    let error = rejected.rejection().await;
    assert!(error.contains("project owner"), "{error}");

    owner.initialized().await;
    let added = owner
        .call(
            "roadmap_add_chunk",
            json!({
                "id":"acknowledged", "title":"Owner mutation survives restart", "group":"Tests"
            }),
        )
        .await;
    assert_eq!(added["id"], "acknowledged", "{added}");
    let roadmap = data.join("roadmap/sessions/owner-test.json");
    let acknowledged = std::fs::read(&roadmap).expect("persisted mutation");
    let mut late = Server::spawn(tmp.path(), &data, &tmp.path().join("late.sock")).await;
    assert!(
        late.response().await.is_none(),
        "no MCP handshake on rejection"
    );
    assert!(late.rejection().await.contains("project owner"));
    assert_eq!(
        std::fs::read(&roadmap).expect("unchanged store"),
        acknowledged
    );

    use std::os::unix::fs::MetadataExt;
    let lock_path = data.join("roadmap/sessions/owner-test.mcp-owner.lock");
    let lock_inode = std::fs::metadata(&lock_path).expect("owner lock").ino();
    owner.child.kill().await.expect("kill owner");
    owner.child.wait().await.expect("owner reaped");
    let mut restarted = Server::spawn(tmp.path(), &data, &tmp.path().join("restart.sock")).await;
    assert!(
        restarted.response().await.is_some(),
        "owner lock releases on process exit"
    );
    assert_eq!(
        std::fs::metadata(&lock_path).expect("same lock file").ino(),
        lock_inode
    );
    restarted.initialized().await;
    let fresh = restarted
        .call("roadmap_get", json!({"ids":["acknowledged"]}))
        .await;
    assert_eq!(fresh["records"][0]["id"], "acknowledged", "{fresh}");
    let status = restarted.call("think_engine_status", json!({})).await;
    assert_eq!(
        status["version"], "0.6.0",
        "fixture targets Cargo.lock: {status}"
    );
    restarted.child.kill().await.expect("stop server");
    restarted.child.wait().await.expect("reap server");
}

#[tokio::test]
async fn occupied_feed_is_preserved_and_failed_start_releases_ownership() {
    let tmp = tempfile::tempdir().expect("tempdir");
    identity(tmp.path());
    let data = tmp.path().join("data");
    let socket = tmp.path().join("feed.sock");
    let listener = std::os::unix::net::UnixListener::bind(&socket).expect("existing feed");
    use std::os::unix::fs::MetadataExt;
    let inode = std::fs::metadata(&socket).expect("socket metadata").ino();
    let mut rejected = Server::spawn(tmp.path(), &data, &socket).await;
    assert!(
        rejected.response().await.is_none(),
        "occupied publisher must refuse startup"
    );
    let error = rejected.rejection().await;
    assert!(error.contains("live-feed socket"), "{error}");
    assert_eq!(
        std::fs::metadata(&socket).expect("preserved socket").ino(),
        inode
    );
    assert!(!data.join("roadmap/sessions/owner-test.json").exists());
    drop(listener);

    // A stale socket left by a dead owner is safe to reclaim.
    let mut recovered = Server::spawn(tmp.path(), &data, &socket).await;
    assert!(
        recovered.response().await.is_some(),
        "failed startup released ownership"
    );
    recovered.child.kill().await.expect("stop recovered server");
    recovered.child.wait().await.expect("reap server");
}

#[tokio::test]
async fn different_stores_racing_for_one_feed_admit_only_one_publisher() {
    let tmp = tempfile::tempdir().expect("tempdir");
    identity(tmp.path());
    let socket = tmp.path().join("shared.sock");
    let mut a = Server::spawn(tmp.path(), &tmp.path().join("data-a"), &socket).await;
    let mut b = Server::spawn(tmp.path(), &tmp.path().join("data-b"), &socket).await;
    let (ra, rb) = tokio::join!(a.response(), b.response());
    assert_ne!(ra.is_some(), rb.is_some(), "exactly one publisher");
    let (mut owner, mut rejected) = if ra.is_some() { (a, b) } else { (b, a) };
    let error = rejected.rejection().await;
    assert!(error.contains("live-feed socket owner"), "{error}");
    owner.initialized().await;
    let result = owner.call("roadmap_status", json!({})).await;
    assert_eq!(result["project_id"], "owner-test");
    owner.child.kill().await.expect("stop owner");
    owner.child.wait().await.expect("reap owner");
}

#[tokio::test]
async fn unbindable_feed_refuses_stdio_tools_without_deleting_the_path() {
    let tmp = tempfile::tempdir().expect("tempdir");
    identity(tmp.path());
    let socket = tmp.path().join("feed.sock");
    std::fs::write(&socket, "existing file").expect("blocking path");
    let mut rejected = Server::spawn(tmp.path(), &tmp.path().join("data"), &socket).await;
    assert!(
        rejected.response().await.is_none(),
        "bind failure must fail startup"
    );
    assert!(rejected.rejection().await.contains("live-feed socket"));
    assert_eq!(
        std::fs::read_to_string(socket).expect("preserved file"),
        "existing file"
    );
}
