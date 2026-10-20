//! Real stdio subprocess regressions for cooperative project-owner admission.
#![cfg(unix)]

#[path = "support/magistr_command.rs"]
mod magistr_command;
#[path = "support/unified.rs"]
mod unified;

use std::path::Path;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::time::{Duration, timeout};

struct Server {
    child: Child,
    // Child::wait closes stdin when Child still owns it. Keep the client
    // connection open while observing rejection or owner death, so waiting
    // cannot manufacture a competing, successful client-disconnect exit.
    stdin: ChildStdin,
    lines: Lines<BufReader<ChildStdout>>,
    last_request: String,
}

impl Server {
    async fn spawn(root: &Path, data: &Path, socket: &Path) -> Self {
        Self::spawn_as(root, data, socket, "default", &["mcp"]).await
    }

    async fn spawn_as(
        root: &Path,
        data: &Path,
        socket: &Path,
        execution: &str,
        args: &[&str],
    ) -> Self {
        let mut child = Command::from(magistr_command::magistr_command())
            .args(args)
            .env("MAGISTR_EXECUTION_ID", execution)
            .env("MAGISTR_WORKSPACE_SESSION_ID", execution)
            .env("MAGISTR_WORKSPACE_NETWORK_FS", "0")
            .env_remove("MAGISTR_WORKSPACE_USER_CONFIG")
            .env_remove("MAGISTR_WORKSPACE_ORG_POLICY")
            .current_dir(root)
            .env("MAGISTR_WORKFLOW_DATA_DIR", data)
            .env("MAGISTR_WORKFLOW_PERSIST", "true")
            .env("MAGISTR_WORKFLOW_BROADCAST_PATH", socket)
            // The store under a tempdir is too deep for `sun_path`: the owner
            // endpoint lands in the runtime directory, and `/tmp` always fits.
            .env("XDG_RUNTIME_DIR", "/tmp")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn MCP server");
        let lines = BufReader::new(child.stdout.take().expect("stdout")).lines();
        let stdin = child.stdin.take().expect("stdin");
        let mut server = Self {
            child,
            stdin,
            lines,
            last_request: String::new(),
        };
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
        self.last_request = value.to_string();
        let bytes = format!("{value}\n");
        self.stdin
            .write_all(bytes.as_bytes())
            .await
            .expect("request");
    }

    async fn response(&mut self) -> Option<Value> {
        timeout(Duration::from_secs(15), self.lines.next_line())
            .await
            .unwrap_or_else(|error| {
                panic!(
                    "server {:?} did not answer {} within 15 seconds: {error}",
                    self.child.id(),
                    self.last_request
                )
            })
            .expect("stdout line")
            .map(|line| serde_json::from_str(&line).expect("MCP JSON"))
    }

    async fn initialized(&mut self) {
        self.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .await;
    }

    async fn call(&mut self, name: &str, arguments: Value) -> Value {
        self.call_with_errors(name, arguments, false).await
    }

    async fn call_with_errors(
        &mut self,
        name: &str,
        arguments: Value,
        expected_error: bool,
    ) -> Value {
        let (public, arguments, composed) = unified::request(name, arguments);
        self.send(json!({"jsonrpc":"2.0","id":2,"method":"tools/call",
            "params":{"name":public,"arguments":arguments}}))
            .await;
        loop {
            let response = self.response().await.expect("tool response");
            if response["id"] == 2 {
                let result = &response["result"];
                assert_eq!(result["isError"] == true, expected_error, "{response}");
                let body =
                    result.get("structuredContent").cloned().unwrap_or_else(|| {
                        serde_json::from_str(result["content"][0]["text"].as_str().unwrap_or_else(
                            || panic!("Missing tool result for {name}: {response}"),
                        ))
                        .expect("structured JSON")
                    });
                return unified::value(body, composed);
            }
        }
    }

    async fn rejection(&mut self) -> String {
        let status = timeout(Duration::from_secs(15), self.child.wait())
            .await
            .expect("bounded rejection")
            .expect("exit status");
        let mut stderr = String::new();
        self.child
            .stderr
            .take()
            .expect("stderr")
            .read_to_string(&mut stderr)
            .await
            .expect("stderr text");
        assert!(
            !status.success(),
            "rejected server must exit unsuccessfully: {status}; stderr: {stderr}"
        );
        stderr
    }

    async fn disconnect(mut self) {
        drop(self.stdin);
        let status = timeout(Duration::from_secs(15), self.child.wait())
            .await
            .expect("bounded client disconnect")
            .expect("exit status");
        assert!(status.success(), "client EOF exits cleanly: {status}");
    }
}

fn identity(root: &Path) {
    std::fs::create_dir_all(root.join(".magistr")).expect("identity directory");
    std::fs::write(
        root.join(".magistr/project.json"),
        "{\"project\":{\"id\":\"owner-test\"}}\n",
    )
    .expect("identity");
}

/// A `.magistr/project.json` selecting the native roadmap, so one-shot
/// commands resolve the same store the owner serves.
fn native_project(root: &Path) {
    std::fs::create_dir_all(root.join(".magistr")).expect("project directory");
    std::fs::write(
        root.join(".magistr/project.json"),
        json!({
            "project": {"id": "owner-test", "name": "owner-test", "description": "", "type": "cli"},
            "stack": {"language":"rust","framework":"none","package_manager":"cargo","test_framework":"cargo-test","linter":"clippy","formatter":"rustfmt"},
            "format": {"command":"true","extensions":["rs"]},
            "testing": {"companion_pattern":"{name}.tests.{ext}","test_dir":"alongside"},
            "reference_repos": [],
            "state_prefix": "fixture",
            "roadmap_exports": false,
            "quality_gate": {"check": "true", "test": "true", "lint": "true"},
            "roadmap_backend": "native"
        })
        .to_string(),
    )
    .expect("project config");
}

#[tokio::test]
async fn next_loop_reads_progress_beside_a_live_mcp_owner() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    identity(root);
    native_project(root);
    let data = root.join("data");
    let mut owner = Server::spawn(root, &data, &root.join("feed.sock")).await;
    assert!(owner.response().await.is_some());
    owner.initialized().await;
    let before = owner.call("magistr_context", json!({})).await;

    for headless in [false, true] {
        owner
            .call(
                "roadmap_add_chunk",
                json!({
                    "id": if headless { "later-two" } else { "later-one" },
                    "title": "Work saved for later", "status": "backlog"
                }),
            )
            .await;
        let mut command = Command::from(magistr_command::magistr_command());
        command
            .args(["next-loop", "--agent", "codex", "--trust", "yolo"])
            // This command must finish without any provider installed.
            .env("PATH", root.join("no-agent-binaries"))
            .current_dir(root)
            .env("HOME", root)
            .env("XDG_CONFIG_HOME", root.join("config"))
            .env("MAGISTR_WORKFLOW_DATA_DIR", &data)
            .env("MAGISTR_WORKFLOW_PERSIST", "true")
            .env("MAGISTR_EXECUTION_ID", "default")
            .stdin(std::process::Stdio::null())
            .kill_on_drop(true);
        if headless {
            command.args(["--headless", "--format", "json"]);
        }
        let output = timeout(Duration::from_secs(30), command.output())
            .await
            .expect("loop startup finishes without dispatch")
            .unwrap();
        assert!(
            output.status.success(),
            "headless={headless}: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let saved = magistr_core::workflow::iteration::IterationState::read(root)
            .unwrap()
            .unwrap();
        assert_eq!(saved.spec_total, if headless { 2 } else { 1 });
        assert_eq!(
            saved.run.terminal_reason,
            Some(magistr_engine::workflow::StopReason::NoReadyChunk)
        );
        assert!(
            saved.operations.records().is_empty(),
            "no provider launched"
        );
        assert!(owner.child.try_wait().unwrap().is_none());
        let after = owner.call("magistr_context", json!({})).await;
        assert_eq!(after["owner_epoch"], before["owner_epoch"]);
    }
    owner.disconnect().await;
}

#[tokio::test]
async fn simultaneous_stdio_start_has_one_owner_and_kill_allows_fresh_restart() {
    let tmp = tempfile::tempdir().expect("tempdir");
    identity(tmp.path());
    let data = tmp.path().join("data");
    // Distinct sockets cannot bypass store ownership: the loser of the race
    // is refused the lease, and then either attaches to the winner's owner
    // endpoint (answering the handshake as a proxy) or, when it asked
    // before the endpoint was up, stands down naming both refusals. It
    // never becomes a second owner.
    let a_socket = tmp.path().join("a.sock");
    let b_socket = tmp.path().join("b.sock");
    let mut a = Server::spawn(tmp.path(), &data, &a_socket).await;
    let mut b = Server::spawn(tmp.path(), &data, &b_socket).await;
    let (ra, rb) = tokio::join!(a.response(), b.response());
    assert!(ra.is_some() || rb.is_some(), "one owner: {ra:?}, {rb:?}");
    // A proxy also answers initialize. Only the admitted owner binds its
    // requested feed, so observe that evidence instead of assuming a won.
    let a_owns = a_socket.exists();
    assert_ne!(a_owns, b_socket.exists(), "exactly one feed owner");
    let (mut owner, mut other) = if a_owns {
        assert!(ra.is_some(), "the owner answered initialize");
        (a, b)
    } else {
        assert!(rb.is_some(), "the owner answered initialize");
        (b, a)
    };
    if ra.is_some() && rb.is_some() {
        other.child.kill().await.expect("stop the attached proxy");
        other.child.wait().await.expect("reap proxy");
    } else {
        let error = other.rejection().await;
        assert!(error.contains("project owner"), "{error}");
        assert!(error.contains("owner endpoint did not take"), "{error}");
    }

    owner.initialized().await;
    let added = owner
        .call(
            "roadmap_add_chunk",
            json!({
                "id":"acknowledged", "title":"Owner mutation survives restart", "group":"Tests"
            }),
        )
        .await;
    assert_eq!(added["value"]["id"], "acknowledged", "{added}");
    let roadmap = data.join("roadmap/sessions/owner-test.json");
    let acknowledged = std::fs::read(&roadmap).expect("persisted mutation");
    // A later server attaches to the owner's endpoint and speaks the
    // owner's own MCP session: the same tools, the one admitted writer.
    let mut late = Server::spawn(tmp.path(), &data, &tmp.path().join("late.sock")).await;
    let handshake = late.response().await.expect("the attached server answers");
    assert_eq!(handshake["id"], 1, "{handshake}");
    assert!(handshake["result"]["serverInfo"].is_object(), "{handshake}");
    late.initialized().await;
    let through = late
        .call(
            "roadmap_add_chunk",
            json!({"id":"attached", "title":"Written through the owner", "group":"Tests"}),
        )
        .await;
    assert_eq!(through["value"]["id"], "attached", "{through}");
    // The owner's cached engines carried it out: no reload, no second writer.
    let seen = owner.call("roadmap_get", json!({"ids":["attached"]})).await;
    assert_eq!(seen["records"][0]["id"], "attached", "{seen}");
    assert_ne!(
        std::fs::read(&roadmap).expect("store"),
        acknowledged,
        "the store moved on the owner's write"
    );

    let mut disconnected =
        Server::spawn(tmp.path(), &data, &tmp.path().join("disconnected.sock")).await;
    assert!(disconnected.response().await.is_some(), "client attached");
    disconnected.initialized().await;
    disconnected.disconnect().await;
    let still_live = owner.call("roadmap_get", json!({"ids":["attached"]})).await;
    assert_eq!(still_live["records"][0]["id"], "attached");

    use std::os::unix::fs::MetadataExt;
    let lock_path = data.join("roadmap/sessions/owner-test.project-owner.lock");
    let lock_inode = std::fs::metadata(&lock_path).expect("owner lock").ino();
    owner.child.kill().await.expect("kill owner");
    owner.child.wait().await.expect("owner reaped");
    // The existing client remains connected and admits the next owner on its
    // next call. Its acknowledged mutations are read from the same store.
    let survived = late.call("roadmap_get", json!({"ids":["attached"]})).await;
    assert_eq!(survived["records"][0]["id"], "attached", "{survived}");
    assert!(
        tmp.path().join("late.sock").exists(),
        "survivor owns generation two"
    );
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
        status["version"],
        env!("CARGO_PKG_VERSION"),
        "the served status reports the first-party workflow crate's version: {status}"
    );
    // Orderly owner EOF is survived as well as a crash, with no client restart.
    late.disconnect().await;
    let after_eof = restarted
        .call("roadmap_get", json!({"ids":["attached"]}))
        .await;
    assert_eq!(after_eof["records"][0]["id"], "attached");
    assert!(tmp.path().join("restart.sock").exists());
    restarted.disconnect().await;
}

#[tokio::test]
async fn owner_crash_during_a_check_refuses_once_and_does_not_replay_the_command() {
    let tmp = tempfile::tempdir().expect("tempdir");
    identity(tmp.path());
    native_project(tmp.path());
    let data = tmp.path().join("data");
    let mut owner = Server::spawn(tmp.path(), &data, &tmp.path().join("owner.sock")).await;
    assert!(owner.response().await.is_some());
    owner.initialized().await;
    owner
        .call(
            "ship_set_objective",
            json!({"description":"interruption fixture","verification":"focused"}),
        )
        .await;
    owner
        .call(
            "ship_plan",
            json!({"action":"add","task_id":"interrupted","title":"Wait for a check"}),
        )
        .await;
    owner
        .call("ship_start", json!({"task_id":"interrupted"}))
        .await;
    let mut survivor = Server::spawn(tmp.path(), &data, &tmp.path().join("survivor.sock")).await;
    assert!(survivor.response().await.is_some());
    survivor.initialized().await;
    let started = tmp.path().join("invocations");
    let script = tmp.path().join("once.sh");
    std::fs::write(
        &script,
        format!("printf 'invoked\\n' >> '{}'; sleep 1\n", started.display()),
    )
    .expect("script");
    survivor
        .send(json!({"jsonrpc":"2.0","id":77,"method":"tools/call",
        "params":{"name":"check_work","arguments":{"type":"test","name":"interrupted-check",
            "command":format!("sh '{}'", script.display())}}}))
        .await;
    timeout(Duration::from_secs(10), async {
        while !started.exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("check actually began");
    owner.child.kill().await.expect("kill fixture owner");
    owner.child.wait().await.expect("reap fixture owner");
    let interrupted = survivor
        .response()
        .await
        .expect("interrupted call answered");
    assert_eq!(interrupted["id"], 77, "{interrupted}");
    assert_eq!(interrupted["result"]["isError"], true, "{interrupted}");
    assert_eq!(
        interrupted["result"]["structuredContent"]["effect"],
        "unknown"
    );
    assert_eq!(
        interrupted["result"]["structuredContent"]["recovery"]["action"],
        "read_back"
    );
    let recovered = survivor.call("ship_status", json!({})).await;
    assert_eq!(recovered["active_task"]["id"], "interrupted");
    assert!(
        recovered["checks"].as_array().unwrap().is_empty(),
        "{recovered}"
    );
    // A duplicate answer for 77 would be the next line, instead of this ping.
    survivor
        .send(json!({"jsonrpc":"2.0","id":78,"method":"ping"}))
        .await;
    let ping = survivor.response().await.expect("connection stays open");
    assert_eq!(ping["id"], 78, "{ping}");
    assert_eq!(
        std::fs::read_to_string(&started).unwrap(),
        "invoked\n",
        "no automatic mutation replay"
    );
    survivor.disconnect().await;
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
    assert!(error.contains("project owner"), "{error}");
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

#[tokio::test]
async fn a_reader_command_answers_beside_the_running_owner_without_a_lease() {
    let tmp = tempfile::tempdir().expect("tempdir");
    identity(tmp.path());
    native_project(tmp.path());
    let data = tmp.path().join("data");
    let mut owner = Server::spawn(tmp.path(), &data, &tmp.path().join("feed.sock")).await;
    assert!(owner.response().await.is_some(), "the owner initialized");
    owner.initialized().await;
    let added = owner
        .call(
            "roadmap_add_chunk",
            json!({"id":"visible", "title":"Seen by the reader", "group":"Tests"}),
        )
        .await;
    assert_eq!(added["value"]["id"], "visible", "{added}");

    let lease = data.join("roadmap/sessions/owner-test.project-owner.lock");
    let before = std::fs::read(&lease).expect("the service owns the project");
    // `magistr roadmap status` is a reader: it takes no lease, so it answers
    // while the owner holds the project, and it sees the owner's write.
    let status = Command::from(magistr_command::magistr_command())
        .args(["--json", "roadmap", "status"])
        .arg(tmp.path())
        .env("MAGISTR_WORKFLOW_DATA_DIR", &data)
        .env("MAGISTR_WORKFLOW_PERSIST", "true")
        .output()
        .await
        .expect("run the reader command");
    assert!(
        status.status.success(),
        "reader refused: {}",
        String::from_utf8_lossy(&status.stderr)
    );
    let report: Value =
        serde_json::from_slice(&status.stdout).expect("roadmap_status JSON on stdout");
    assert_eq!(report["status"], "ok", "{report}");
    assert_eq!(report["data"]["project_id"], "owner-test", "{report}");
    assert_eq!(report["data"]["counts"]["pending"], 1, "{report}");
    assert_eq!(report["data"]["next"], "visible", "{report}");
    assert_eq!(
        std::fs::read(&lease).expect("stable owner lease"),
        before,
        "a reader neither acquires nor changes the owner's lease"
    );

    // The owner is untouched by the read.
    let after = owner.call("roadmap_status", json!({})).await;
    assert_eq!(after["project_id"], "owner-test");
    owner.child.kill().await.expect("stop owner");
    owner.child.wait().await.expect("reap owner");
}
fn git(root: &Path, args: &[&str]) {
    let result = std::process::Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .expect("git");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

async fn ready(server: &mut Server) {
    assert!(server.response().await.expect("initialize")["result"].is_object());
    server.initialized().await;
}

async fn begin_work(server: &mut Server, scope: &str) {
    server
        .call(
            "ship_set_objective",
            json!({"description": scope, "scope": scope, "verification":"focused"}),
        )
        .await;
    server
        .call(
            "ship_plan",
            json!({"action":"add","task_id":"verify","title":"Verify source","task_type":"test"}),
        )
        .await;
    server.call("ship_start", json!({"task_id":"verify"})).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn linked_worktrees_share_one_workflow_owner_and_keep_source_checks_and_recovery_separate() {
    use magistr::application::{WorkingCopies, WorkingCopyError};
    use magistr_engine::native::{ProjectAuthority, WriterRole};
    use magistr_workspace::snapshot::{SelectionPolicy, SourceSnapshot};
    let temp = tempfile::tempdir().expect("temporary");
    let main = temp.path().join("main");
    std::fs::create_dir(&main).expect("root");
    identity(&main);
    native_project(&main);
    std::fs::write(
        main.join(".gitignore"),
        ".magistr/workspace/\n.magistr/\n.magistr/traces/\n",
    )
    .expect("ignore");
    std::fs::write(main.join("source.txt"), "original\n").expect("source");
    std::fs::write(
        main.join(".magistr/workspace.toml"),
        "[edit]\nconfirm_mode = \"token\"\nforce_confirm = true\n",
    )
    .expect("configuration");
    git(&main, &["init", "-b", "main"]);
    git(&main, &["config", "user.name", "Test"]);
    git(&main, &["config", "user.email", "test@example.invalid"]);
    git(&main, &["add", "."]);
    git(&main, &["commit", "-m", "initial"]);
    let copies = WorkingCopies::at(&main).expect("working copies");
    let linked = copies
        .create(&temp.path().join("task-b"), "task-b", "HEAD")
        .expect("create")
        .path;
    let main = main.canonicalize().expect("canonical");
    assert!(!linked.join(".magistr/project.json").exists());
    let data = temp.path().join("data");
    let socket = temp.path().join("feed.sock");

    let mut a = Server::spawn_as(&main, &data, &socket, "task-a", &["mcp"]).await;
    ready(&mut a).await;
    let mut b = Server::spawn_as(&linked, &data, &socket, "task-b", &["mcp"]).await;
    ready(&mut b).await;
    begin_work(&mut a, "chunk:a").await;
    begin_work(&mut b, "chunk:b").await;
    let status_a = a.call("ship_status", json!({})).await;
    let status_b = b.call("ship_status", json!({})).await;
    assert_eq!(status_a["objective"]["scope"], "chunk:a");
    assert_eq!(status_b["objective"]["scope"], "chunk:b");
    assert_eq!(status_a["execution"]["working_copy"], json!(main));
    assert_eq!(status_b["execution"]["working_copy"], json!(linked));
    assert_eq!(
        status_a["execution"]["owner_generation"],
        status_b["execution"]["owner_generation"]
    );
    assert_ne!(status_a["run_id"], status_b["run_id"]);
    let cfg = magistr_core::evidence::writer_config(&linked, |_| None).with_data_dir(&data);
    for role in [WriterRole::Host, WriterRole::Service] {
        assert_eq!(
            ProjectAuthority::admit(&cfg, role)
                .expect_err("one workflow writer")
                .kind(),
            std::io::ErrorKind::WouldBlock
        );
    }

    let mut source_a =
        Server::spawn_as(&main, &data, &socket, "same-source-session", &["mcp"]).await;
    let mut source_b =
        Server::spawn_as(&linked, &data, &socket, "same-source-session", &["mcp"]).await;
    ready(&mut source_a).await;
    ready(&mut source_b).await;
    // Equal client-supplied session names still cannot transfer an approval
    // between different source owners and canonical roots.
    let mut request = json!({"op":"move","from":"source.txt","to":"moved.txt"});
    let preview = source_a
        .call_with_errors("files", request.clone(), true)
        .await;
    assert_eq!(preview["error"]["code"], "CONFIRM_REQUIRED", "{preview}");
    request["confirm"] = preview["error"]["confirmToken"].clone();
    let denied = source_b.call_with_errors("files", request, true).await;
    assert_eq!(denied["error"]["code"], "CONFIRM_MISMATCH", "{denied}");
    assert!(!linked.join("moved.txt").exists());
    for (server, text) in [(&mut source_a, "tree a\n"), (&mut source_b, "tree b\n")] {
        let write = server
            .call("write", json!({"path":"source.txt","content":text}))
            .await;
        assert_eq!(write["ok"], true, "{write}");
    }
    assert_eq!(
        std::fs::read_to_string(main.join("source.txt")).expect("a"),
        "tree a\n"
    );
    assert_eq!(
        std::fs::read_to_string(linked.join("source.txt")).expect("b"),
        "tree b\n"
    );
    assert!(matches!(
        copies.archive(&linked),
        Err(WorkingCopyError::Active)
    ));

    for (server, expected) in [(&mut a, "tree a"), (&mut b, "tree b")] {
        let check = server
            .call(
                "ship_check",
                json!({"type":"test","name":"source","command":"cat source.txt"}),
            )
            .await;
        assert_eq!(check["check"]["value"]["passed"], true, "{check}");
        assert!(
            check["check"]["value"]["details"]
                .as_str()
                .expect("details")
                .contains(expected),
            "{check}"
        );
    }
    let gate_a = a.call("magistr_gate", json!({"gate":"test"})).await;
    let gate_b = b.call("magistr_gate", json!({"gate":"test"})).await;
    assert_eq!(gate_a["passed"], true, "{gate_a}");
    assert_eq!(gate_b["passed"], true, "{gate_b}");
    let receipt_a = a
        .call("magistr_context", json!({"receipt_id":gate_a["receipt"]}))
        .await;
    let receipt_b = b
        .call("magistr_context", json!({"receipt_id":gate_b["receipt"]}))
        .await;
    for (receipt, status, root) in [
        (&receipt_a, &status_a, &main),
        (&receipt_b, &status_b, &linked),
    ] {
        assert_eq!(
            receipt["receipt"]["target"]["execution"],
            status["execution"]
        );
        assert_eq!(receipt["receipt"]["target"]["run_id"], status["run_id"]);
        assert_eq!(receipt["receipt"]["source"]["workspace_root"], json!(root));
    }
    assert!(
        a.call("magistr_context", json!({"receipt_id":gate_b["receipt"]}))
            .await["receipt"]
            .is_null(),
        "no receipt cross-delivery"
    );
    std::fs::write(main.join("source.txt"), "changed after check\n").expect("change a");
    let witness_b =
        SourceSnapshot::capture(&linked, SelectionPolicy::default()).expect("b observation");
    assert_eq!(
        receipt_b["receipt"]["source"]["revision"],
        witness_b.revision(),
        "a does not invalidate b"
    );
    assert_ne!(
        receipt_a["receipt"]["source"]["revision"],
        SourceSnapshot::capture(&main, SelectionPolicy::default())
            .expect("a")
            .revision()
    );

    let generation = status_b["execution"]["owner_generation"]
        .as_u64()
        .expect("generation");
    a.child.kill().await.expect("kill owner");
    timeout(Duration::from_secs(15), a.child.wait())
        .await
        .expect("bounded reap")
        .expect("reap");
    // An existing bridge re-admits on its next call, with its own context.
    let recovered_b = b.call("ship_status", json!({})).await;
    assert_eq!(recovered_b["objective"]["scope"], "chunk:b");
    assert_eq!(recovered_b["run_id"], status_b["run_id"]);
    assert!(
        recovered_b["execution"]["owner_generation"]
            .as_u64()
            .expect("new generation")
            > generation
    );
    let mut recovered_a = Server::spawn_as(&main, &data, &socket, "task-a", &["mcp"]).await;
    ready(&mut recovered_a).await;
    assert_eq!(
        recovered_a.call("ship_status", json!({})).await["objective"]["scope"],
        "chunk:a"
    );
    source_a.disconnect().await;
    source_b.disconnect().await;
    recovered_a.disconnect().await;
    b.disconnect().await;
    ProjectAuthority::admit(&cfg, WriterRole::Host).expect("all workflow connections released");
    let nested = linked.join("app");
    std::fs::create_dir(&nested).expect("nested project");
    let mut nested_source = Server::spawn_as(&nested, &data, &socket, "nested", &["mcp"]).await;
    ready(&mut nested_source).await;
    nested_source.call("workspace_status", json!({})).await;
    assert!(
        matches!(copies.archive(&linked), Err(WorkingCopyError::Active)),
        "nested source owner keeps the Git copy active"
    );
    nested_source.disconnect().await;
    assert!(
        matches!(copies.archive(&linked), Err(WorkingCopyError::Dirty)),
        "cleanup preserves unreviewed work"
    );
}
