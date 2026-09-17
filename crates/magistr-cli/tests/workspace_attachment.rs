//! Native stdio processes share one admitted workspace owner, retain distinct
//! sessions and outlive that owner: a bridge whose owner disappears answers
//! its calls with the typed refusal and reconnects on the next request.
#![cfg(unix)]
mod workspace_attachment_support;

use serde_json::json;
use std::{process::Stdio, sync::Arc};
use tokio::{sync::Barrier, task::JoinSet};
use workspace_attachment_support::{
    Connection, WAIT, assert_ok, binary, command, dead_legacy_record, deny_admission, edit,
    file_text, generation, hostname, magistr, migrate, read_text, restore_admission, workspace,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_second_process_initializes_against_the_live_workspace_owner() {
    let dir = workspace();
    let owner = Connection::open(dir.path()).await;
    let attached = Connection::open(dir.path()).await;
    attached.assert_tools().await;
    assert_eq!(generation(dir.path()), 1);
    attached.close().await;
    assert_ok(&owner.call("workbench_status", json!({})).await);
    owner.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn eight_simultaneous_clients_admit_exactly_one_owner_and_all_receive_the_tool_surface() {
    let dir = workspace();
    let barrier = Arc::new(Barrier::new(8));
    let mut opening = JoinSet::new();
    for _ in 0..8 {
        let root = dir.path().to_owned();
        let barrier = Arc::clone(&barrier);
        opening.spawn(async move {
            barrier.wait().await;
            Connection::open(&root).await
        });
    }
    let mut connections = Vec::new();
    while let Some(connection) = opening.join_next().await {
        connections.push(connection.expect("client initialization task"));
    }
    assert_eq!(connections.len(), 8);
    for connection in &connections {
        connection.assert_tools().await;
        assert_ok(&connection.call("workbench_status", json!({})).await);
    }
    assert_eq!(
        generation(dir.path()),
        1,
        "startup contenders never open a second service"
    );
    let mut closing = JoinSet::new();
    for Connection { mut child, client } in connections {
        closing.spawn(async move {
            // The winner is intentionally unknown. All transports close together;
            // a relay can observe either its own EOF or the owner's shutdown.
            let _ = tokio::time::timeout(WAIT, client.cancel()).await;
            tokio::time::timeout(WAIT, child.wait())
                .await
                .expect("every raced process exits")
                .expect("reap raced process");
        });
    }
    while let Some(closed) = closing.join_next().await {
        closed.expect("client cleanup task");
    }
    let replacement = Connection::open(dir.path()).await;
    assert_eq!(
        generation(dir.path()),
        2,
        "all clients released the first owner"
    );
    replacement.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn canonical_aliases_attach_but_independent_roots_keep_distinct_owners() {
    let first = workspace();
    let second = workspace();
    let owner = Connection::open(first.path()).await;
    let alias = Connection::open(&first.path().join(".")).await;
    let independent = Connection::open(second.path()).await;
    let canonical = first.path().canonicalize().expect("canonical root");
    for connection in [&owner, &alias] {
        let status = connection.call("workbench_status", json!({})).await;
        assert_ok(&status);
        assert_eq!(
            status["results"][0]["root"],
            canonical.to_string_lossy().as_ref()
        );
    }
    assert_ok(
        &alias
            .call("edit", edit("shared.txt", "first workspace"))
            .await,
    );
    assert_eq!(file_text(first.path(), "shared.txt"), "first workspace\n");
    assert_eq!(file_text(second.path(), "shared.txt"), "original bytes\n");
    assert_eq!(generation(first.path()), 1);
    assert_eq!(generation(second.path()), 1);
    alias.close().await;
    independent.close().await;
    owner.close().await;
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_symlinked_root_uses_the_canonical_owners_endpoint() {
    let dir = workspace();
    let aliases = tempfile::tempdir().expect("alias parent");
    let path = aliases.path().join("workspace alias");
    std::os::unix::fs::symlink(dir.path(), &path).expect("workspace symlink");
    let owner = Connection::open(dir.path()).await;
    let attached = Connection::open(&path).await;
    let status = attached.call("workbench_status", json!({})).await;
    assert_ok(&status);
    assert_eq!(
        status["results"][0]["root"],
        dir.path()
            .canonicalize()
            .expect("canonical root")
            .to_string_lossy()
            .as_ref()
    );
    assert_eq!(generation(dir.path()), 1);
    attached.close().await;
    owner.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn automatic_sessions_isolate_undo_and_concurrent_writes_have_one_current_anchor() {
    let dir = workspace();
    let alice = Connection::open(dir.path()).await;
    let bob = Connection::open(dir.path()).await;
    let (a, b) = tokio::join!(
        alice.call("edit", edit("alice.txt", "alice bytes")),
        bob.call("edit", edit("bob.txt", "bob bytes")),
    );
    assert_ok(&a);
    assert_ok(&b);
    let history = json!({"action": "list", "scope": "session"});
    let a = alice.call("undo", history.clone()).await;
    let b = bob.call("undo", history).await;
    assert_eq!(a["results"].as_array().expect("alice rows").len(), 1);
    assert_eq!(b["results"].as_array().expect("bob rows").len(), 1);
    assert_ne!(a["results"][0]["sessionId"], b["results"][0]["sessionId"]);
    for row in [&a["results"][0], &b["results"][0]] {
        assert!(
            !row["sessionId"]
                .as_str()
                .expect("automatic identity")
                .is_empty()
        );
    }
    assert_eq!(
        read_text(&bob.call("read", json!({"file": "alice.txt"})).await),
        "alice bytes"
    );
    assert_ok(&bob.call("undo", json!({"scope": "session"})).await);
    assert_eq!(file_text(dir.path(), "bob.txt"), "original bytes\n");
    assert_eq!(file_text(dir.path(), "alice.txt"), "alice bytes\n");
    let read = alice.call("read", json!({"file": "shared.txt"})).await;
    assert_ok(&read);
    let hash = read["results"]
        .as_array()
        .expect("read rows")
        .iter()
        .find_map(|row| row["contentHash"].as_str())
        .expect("anchored read");
    let mut a = edit("shared.txt", "alice wins");
    let mut b = edit("shared.txt", "bob wins");
    a["expectedHash"] = json!(hash);
    b["expectedHash"] = json!(hash);
    let (a, b) = tokio::join!(alice.call("edit", a), bob.call("edit", b));
    assert_ne!(a["ok"], b["ok"], "exactly one write applies: {a} / {b}");
    let refused = if a["ok"] == true { &b } else { &a };
    assert_eq!(refused["error"]["code"], "STALE_ANCHOR", "{refused}");
    let expected = if a["ok"] == true {
        "alice wins\n"
    } else {
        "bob wins\n"
    };
    assert_eq!(file_text(dir.path(), "shared.txt"), expected);
    assert_eq!(
        read_text(&alice.call("read", json!({"file": "shared.txt"})).await),
        expected.trim_end()
    );
    assert_eq!(
        read_text(&bob.call("read", json!({"file": "shared.txt"})).await),
        expected.trim_end()
    );
    bob.close().await;
    alice.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn confirmation_and_undo_follow_explicit_identity_across_attached_connections() {
    let dir = workspace();
    let owner = Connection::open(dir.path()).await;
    let attached = Connection::open(dir.path()).await;
    let mut request = json!({"op": "move", "from": "pending.txt", "to": "moved.txt"});
    let preview = owner
        .session_call("files", request.clone(), Some("alice"))
        .await;
    assert_eq!(preview["error"]["code"], "CONFIRM_REQUIRED", "{preview}");
    request["confirm"] = preview["error"]["confirmToken"].clone();
    let denied = attached
        .session_call("files", request.clone(), Some("bob"))
        .await;
    assert_eq!(denied["error"]["code"], "CONFIRM_MISMATCH", "{denied}");
    assert_eq!(file_text(dir.path(), "pending.txt"), "original bytes\n");
    assert!(!dir.path().join("moved.txt").exists());
    assert_ok(&attached.session_call("files", request, Some("alice")).await);
    assert!(!dir.path().join("pending.txt").exists());
    assert_eq!(file_text(dir.path(), "moved.txt"), "original bytes\n");
    let listing = json!({"action": "list", "scope": "session"});
    let rows = attached
        .session_call("undo", listing.clone(), Some("bob"))
        .await;
    assert_eq!(rows["results"], json!([]));
    assert_ok(&attached.session_call("undo", json!({}), Some("bob")).await);
    assert!(dir.path().join("moved.txt").exists());
    attached.close().await;
    let reconnected = Connection::open(dir.path()).await;
    let rows = reconnected
        .session_call("undo", listing, Some("alice"))
        .await;
    assert!(
        rows["results"]
            .as_array()
            .expect("durable session rows")
            .iter()
            .any(|row| row["sessionId"] == "alice" && row["tool"] == "files")
    );
    let operations: Vec<_> = rows["results"]
        .as_array()
        .expect("journal operations")
        .iter()
        .map(|row| row["seq"].clone())
        .collect();
    assert_ok(
        &reconnected
            .session_call("undo", json!({"ops": operations}), Some("alice"))
            .await,
    );
    assert_eq!(file_text(dir.path(), "pending.txt"), "original bytes\n");
    assert!(!dir.path().join("moved.txt").exists());
    assert_eq!(generation(dir.path()), 1);
    reconnected.close().await;
    owner.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn attached_eof_preserves_the_owner_and_owner_eof_is_survived_by_the_remaining_client() {
    let dir = workspace();
    let owner = Connection::open(dir.path()).await;
    let leaving = Connection::open(dir.path()).await;
    let mut remaining = Connection::open(dir.path()).await;
    leaving.close().await;
    assert_ok(&owner.call("workbench_status", json!({})).await);
    assert_ok(&remaining.call("read", json!({"file": "shared.txt"})).await);
    owner.close().await;
    assert_eq!(
        generation(dir.path()),
        1,
        "nothing admits until a client asks"
    );
    remaining.assert_alive();
    // The next request reconnects: nobody owns the workspace, so this bridge
    // admits it and keeps serving the same tool surface.
    assert_ok(&remaining.call("read", json!({"file": "shared.txt"})).await);
    assert_eq!(generation(dir.path()), 2);
    remaining.assert_tools().await;
    remaining.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn owner_crash_is_survived_by_attached_clients_which_reconnect_on_their_next_call() {
    let dir = workspace();
    let mut owner = Connection::open(dir.path()).await;
    let mut attached = Connection::open(dir.path()).await;
    let peer = Connection::open(dir.path()).await;
    // This handle belongs only to the scratch owner this test spawned.
    owner.child.start_kill().expect("crash the scratch owner");
    let status = tokio::time::timeout(WAIT, owner.child.wait())
        .await
        .expect("crashed owner exits")
        .expect("reap crashed owner");
    assert!(!status.success());
    let _ = owner.client.cancel().await;
    attached.assert_alive();
    assert_ok(&attached.call("workbench_status", json!({})).await);
    assert_eq!(
        generation(dir.path()),
        2,
        "one client admitted the workspace"
    );
    attached.assert_tools().await;
    // The other survivor attaches to that new owner instead of admitting.
    assert_ok(&peer.call("read", json!({"file": "shared.txt"})).await);
    assert_eq!(generation(dir.path()), 2);
    peer.close().await;
    attached.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn owner_loss_answers_calls_typed_and_keeps_the_tool_surface_until_an_owner_returns() {
    let dir = workspace();
    let mut owner = Connection::open(dir.path()).await;
    let mut attached = Connection::open(dir.path()).await;
    attached.assert_tools().await;
    owner.child.start_kill().expect("crash the scratch owner");
    tokio::time::timeout(WAIT, owner.child.wait())
        .await
        .expect("crashed owner exits")
        .expect("reap crashed owner");
    let _ = owner.client.cancel().await;
    let native = deny_admission(dir.path());
    let refused = attached.call("workbench_status", json!({})).await;
    assert_eq!(refused["ok"], false, "{refused}");
    assert_eq!(refused["error"]["code"], "ENGINE_UNAVAILABLE", "{refused}");
    assert_eq!(refused["error"]["recovery"], "retry", "{refused}");
    attached.assert_tools().await;
    attached.assert_alive();
    assert_eq!(
        generation(dir.path()),
        1,
        "a refused admission mints nothing"
    );
    restore_admission(dir.path(), &native);
    assert_ok(&attached.call("workbench_status", json!({})).await);
    assert_eq!(generation(dir.path()), 2);
    attached.assert_tools().await;
    attached.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_killed_owner_takes_its_worker_with_it() {
    let dir = workspace();
    std::fs::create_dir(dir.path().join("src")).expect("source directory");
    std::fs::write(dir.path().join("src/main.rs"), "fn main() {}\n").expect("source fixture");
    let mut owner = Connection::open(dir.path()).await;
    let owner_pid = owner.child.id().expect("owner pid");
    // The supervisor starts its worker on the first parse it needs.
    assert_ok(
        &owner
            .call(
                "search",
                json!({"query": "fn $NAME() {}", "mode": "structural", "path": "src/**"}),
            )
            .await,
    );
    let worker = tokio::time::timeout(WAIT, async {
        loop {
            if let Some(pid) = worker_of(owner_pid) {
                break pid;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("the owner supervises a worker");
    owner.child.start_kill().expect("crash the scratch owner");
    tokio::time::timeout(WAIT, owner.child.wait())
        .await
        .expect("crashed owner exits")
        .expect("reap crashed owner");
    let _ = owner.client.cancel().await;
    tokio::time::timeout(WAIT, async {
        // SAFETY: signal 0 only observes this fixture's worker; it sends none.
        while unsafe { libc::kill(worker, 0) } == 0 {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("the orphaned worker ends by itself");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_orphaned_worker_whose_stdin_stays_open_ends_by_itself() {
    // A shell parent stands in for a killed owner (the trailing command keeps
    // the shell from exec'ing the worker in its place). The worker's stdin is
    // held open here, so EOF cannot end it: only the parent watch can.
    let mut parent = tokio::process::Command::new("sh")
        .arg("-c")
        .arg("\"$0\" --workspace-worker; exit $?")
        .arg(env!("CARGO_BIN_EXE_magistr"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .expect("shell parent");
    let parent_pid = parent.id().expect("parent pid");
    let worker = tokio::time::timeout(WAIT, async {
        loop {
            if let Some(pid) = worker_of(parent_pid) {
                break pid;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("the shell starts a worker");
    parent.start_kill().expect("kill the stand-in owner");
    tokio::time::timeout(WAIT, parent.wait())
        .await
        .expect("stand-in exits")
        .expect("reap stand-in");
    tokio::time::timeout(WAIT, async {
        // SAFETY: signal 0 only observes this fixture's worker; it sends none.
        while unsafe { libc::kill(worker, 0) } == 0 {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("the re-parented worker ends although its stdin is open");
}

/// The `--workspace-worker` child of `owner`, as the process table shows it.
fn worker_of(owner: u32) -> Option<i32> {
    let table = std::process::Command::new("ps")
        .args(["-axo", "pid=,ppid=,command="])
        .output()
        .expect("process table");
    String::from_utf8_lossy(&table.stdout)
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let pid: i32 = fields.next()?.parse().ok()?;
            let ppid: u32 = fields.next()?.parse().ok()?;
            let command = fields.collect::<Vec<_>>().join(" ");
            (ppid == owner && command.contains("--workspace-worker")).then_some(pid)
        })
        .next()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_bridge_keeps_its_session_identity_across_owner_change() {
    let dir = workspace();
    let mut owner = Connection::open(dir.path()).await;
    let attached = Connection::open(dir.path()).await;
    assert_ok(&attached.call("edit", edit("alice.txt", "alice v1")).await);
    let before = attached
        .call("undo", json!({"action": "list", "scope": "session"}))
        .await;
    assert_ok(&before);
    assert_eq!(before["results"].as_array().map(Vec::len), Some(1));
    owner.child.start_kill().expect("crash the scratch owner");
    tokio::time::timeout(WAIT, owner.child.wait())
        .await
        .expect("crashed owner exits")
        .expect("reap crashed owner");
    let _ = owner.client.cancel().await;
    let after = attached
        .call("undo", json!({"action": "list", "scope": "session"}))
        .await;
    assert_ok(&after);
    assert_eq!(generation(dir.path()), 2);
    assert_eq!(
        after["results"], before["results"],
        "the durable identity still selects its own history under the new owner"
    );
    assert_ok(&attached.call("edit", edit("bob.txt", "bob v1")).await);
    let grown = attached
        .call("undo", json!({"action": "list", "scope": "session"}))
        .await;
    assert_eq!(grown["results"].as_array().map(Vec::len), Some(2));
    // A default rewind takes the identity's latest batch, journaled under the
    // new owner; the older op under the old owner stays selectable.
    assert_ok(&attached.call("undo", json!({"action": "rewind"})).await);
    assert_eq!(file_text(dir.path(), "bob.txt"), "original bytes\n");
    assert_eq!(file_text(dir.path(), "alice.txt"), "alice v1\n");
    attached.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_attached_client_cannot_change_the_live_owners_configuration() {
    let dir = workspace();
    let owner = Connection::open(dir.path()).await;
    let output = tokio::time::timeout(
        WAIT,
        command(dir.path())
            .args(["--config", "edit.confirm_mode=token"])
            .stdin(Stdio::null())
            .output(),
    )
    .await
    .expect("configuration refusal is bounded")
    .expect("configured contender");
    assert!(
        !output.status.success(),
        "an attachment cannot replace owner configuration"
    );
    assert!(output.stdout.is_empty(), "refusal stays off MCP stdout");
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(diagnostic.contains("config"), "{diagnostic}");
    assert_ok(&owner.call("workbench_status", json!({})).await);
    assert_eq!(generation(dir.path()), 1);
    owner.close().await;
}

/// The lock is held by an owner that never publishes an endpoint: each
/// attachment attempt is bounded by the fixed deadline, the bridge serves
/// discovery meanwhile instead of exiting, opens no store, and admits the
/// workspace itself once the lock is released.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_bridge_outlives_a_native_owner_without_an_endpoint_and_admits_once_the_lock_is_released()
{
    let dir = workspace();
    let state = dir.path().join(".workbench");
    std::fs::create_dir(&state).expect("owner state directory");
    let held = magistr_workspace::service::ServerLock::acquire(state.join("server.lock"))
        .expect("hold native admission without an attachment listener");
    let marker = std::fs::read(held.path()).expect("native marker");
    let admitted_generation = held.generation().get();
    let started = std::time::Instant::now();
    // Startup's attempt and the one the client's `initialize` triggers are
    // each bounded by the five-second attachment deadline.
    let mut deferred = Connection::open(dir.path()).await;
    assert!(
        started.elapsed() < std::time::Duration::from_secs(20),
        "initialize was answered after {:?}",
        started.elapsed()
    );
    let refused = deferred.call("workbench_status", json!({})).await;
    assert_eq!(refused["ok"], false, "{refused}");
    assert_eq!(refused["error"]["code"], "ENGINE_UNAVAILABLE", "{refused}");
    assert_eq!(refused["error"]["recovery"], "retry", "{refused}");
    let message = refused["error"]["message"].as_str().expect("message");
    assert!(message.contains("attach"), "{message}");
    assert!(
        message.contains("timed out") || message.contains("timeout"),
        "{message}"
    );
    deferred.assert_alive();
    assert_eq!(generation(dir.path()), admitted_generation);
    assert_eq!(
        std::fs::read(held.path()).expect("preserved marker"),
        marker
    );
    for store in [
        "journal.db",
        "index.db",
        "native-index.db",
        "keys",
        "trace.jsonl",
    ] {
        assert!(
            !state.join(store).exists(),
            "refused attachment opened {store}"
        );
    }
    drop(held);
    assert_ok(&deferred.call("workbench_status", json!({})).await);
    assert_eq!(generation(dir.path()), admitted_generation + 1);
    deferred.assert_tools().await;
    deferred.close().await;
}

/// The marker still holds the legacy server's record: the bridge starts,
/// answers `initialize` and the tool listing from the shared definition,
/// refuses calls typed with the migration as the named action, and after
/// `magistr workspace migrate` its next call admits — the handshake and the
/// listing it gave are the ones a live owner gives.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_bridge_against_an_unmigrated_marker_serves_discovery_and_admits_after_migration() {
    let dir = workspace();
    let state = dir.path().join(".workbench");
    std::fs::create_dir(&state).expect("state");
    let lock = state.join("server.lock");
    let record = dead_legacy_record();
    std::fs::write(&lock, &record).expect("legacy record");
    let mut deferred = Connection::open(dir.path()).await;
    deferred.assert_tools().await;
    let listed_before = tokio::time::timeout(WAIT, deferred.client.peer().list_tools(None))
        .await
        .expect("bounded tools/list")
        .expect("the shared definition lists");
    let refused = deferred.call("workbench_status", json!({})).await;
    assert_eq!(refused["ok"], false, "{refused}");
    assert_eq!(refused["error"]["code"], "ENGINE_UNAVAILABLE", "{refused}");
    assert_eq!(refused["error"]["recovery"], "retry", "{refused}");
    let message = refused["error"]["message"].as_str().expect("message");
    assert!(message.contains("magistr workspace migrate"), "{message}");
    deferred.assert_alive();
    assert!(
        !state.join("server.generation").exists(),
        "a refused admission mints nothing"
    );
    assert_eq!(std::fs::read_to_string(&lock).expect("preserved"), record);

    let migrated = migrate(dir.path()).await;
    assert_eq!(
        migrated.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&migrated.stderr)
    );
    assert_ok(&deferred.call("workbench_status", json!({})).await);
    assert_eq!(
        generation(dir.path()),
        1,
        "the bridge admitted on its next call"
    );
    assert_ok(&deferred.call("read", json!({"file": "shared.txt"})).await);

    let live = Connection::open(dir.path()).await;
    assert_eq!(
        deferred.client.peer().peer_info(),
        live.client.peer().peer_info(),
        "the deferred handshake is the owner's handshake"
    );
    let listed_after = tokio::time::timeout(WAIT, live.client.peer().list_tools(None))
        .await
        .expect("bounded tools/list")
        .expect("the owner lists");
    assert_eq!(
        serde_json::to_value(&listed_before).expect("listing"),
        serde_json::to_value(&listed_after).expect("listing"),
        "the deferred listing is the owner's listing"
    );
    live.close().await;
    deferred.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn attachment_rejects_a_confirmation_after_external_bytes_change() {
    let dir = workspace();
    let owner = Connection::open(dir.path()).await;
    let attached = Connection::open(dir.path()).await;
    let mut request = json!({"op": "move", "from": "pending.txt", "to": "moved.txt"});
    let preview = owner
        .session_call("files", request.clone(), Some("alice"))
        .await;
    assert_eq!(preview["error"]["code"], "CONFIRM_REQUIRED", "{preview}");
    request["confirm"] = preview["error"]["confirmToken"].clone();
    std::fs::write(dir.path().join("pending.txt"), "external editor bytes\n")
        .expect("external edit after preview");
    let denied = attached.session_call("files", request, Some("alice")).await;
    assert_eq!(denied["error"]["code"], "CONFIRM_MISMATCH", "{denied}");
    assert_eq!(
        file_text(dir.path(), "pending.txt"),
        "external editor bytes\n"
    );
    assert!(!dir.path().join("moved.txt").exists());
    let listing = attached
        .session_call(
            "undo",
            json!({"action": "list", "scope": "session"}),
            Some("alice"),
        )
        .await;
    assert_ok(&listing);
    assert_eq!(
        listing["results"],
        json!([]),
        "stale approval journals nothing"
    );
    attached.close().await;
    owner.close().await;
}

#[path = "workspace_attachment_support/shutdown.tests.rs"]
mod shutdown_tests;

#[path = "workspace_attachment_support/launcher.tests.rs"]
mod launcher_tests;
