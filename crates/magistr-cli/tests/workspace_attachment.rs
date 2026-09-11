//! Native stdio processes share one admitted workspace owner, retain distinct
//! sessions and end when their invocation-owned server disappears.
#![cfg(unix)]
mod workspace_attachment_support;

use serde_json::json;
use std::{process::Stdio, sync::Arc};
use tokio::{sync::Barrier, task::JoinSet};
use workspace_attachment_support::{
    Connection, WAIT, assert_ok, command, edit, file_text, generation, read_text, workspace,
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
async fn attached_eof_preserves_the_owner_and_owner_eof_closes_remaining_attachments() {
    let dir = workspace();
    let owner = Connection::open(dir.path()).await;
    let leaving = Connection::open(dir.path()).await;
    let remaining = Connection::open(dir.path()).await;
    leaving.close().await;
    assert_ok(&owner.call("workbench_status", json!({})).await);
    assert_ok(&remaining.call("read", json!({"file": "shared.txt"})).await);
    owner.close().await;
    remaining.assert_owner_lost().await;
    assert_eq!(
        generation(dir.path()),
        1,
        "disconnect never starts a replacement"
    );
    let replacement = Connection::open(dir.path()).await;
    assert_eq!(generation(dir.path()), 2);
    replacement.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn owner_crash_closes_attachments_and_only_an_explicit_new_invocation_restarts() {
    let dir = workspace();
    let mut owner = Connection::open(dir.path()).await;
    let attached = Connection::open(dir.path()).await;
    // This handle belongs only to the scratch owner this test spawned.
    owner.child.start_kill().expect("crash the scratch owner");
    let status = tokio::time::timeout(WAIT, owner.child.wait())
        .await
        .expect("crashed owner exits")
        .expect("reap crashed owner");
    assert!(!status.success());
    attached.assert_owner_lost().await;
    let _ = owner.client.cancel().await;
    assert_eq!(
        generation(dir.path()),
        1,
        "attached clients never replace a lost owner"
    );
    let replacement = Connection::open(dir.path()).await;
    assert_eq!(generation(dir.path()), 2);
    replacement.assert_tools().await;
    replacement.close().await;
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_native_owner_without_an_endpoint_refuses_attachment_within_a_fixed_deadline() {
    let dir = workspace();
    let state = dir.path().join(".workbench");
    std::fs::create_dir(&state).expect("owner state directory");
    let held = magistr_workspace::service::ServerLock::acquire(state.join("server.lock"))
        .expect("hold native admission without an attachment listener");
    let marker = std::fs::read(held.path()).expect("native marker");
    let admitted_generation = held.generation().get();
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(8),
        command(dir.path())
            .kill_on_drop(true)
            .stdin(Stdio::null())
            .output(),
    )
    .await
    .expect("attachment to a non-serving owner is bounded")
    .expect("attachment contender exits");
    assert_eq!(output.status.code(), Some(75));
    assert!(
        output.stdout.is_empty(),
        "admission diagnostics never reach MCP stdout"
    );
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(diagnostic.contains("attach"), "{diagnostic}");
    assert!(
        diagnostic.contains("timed out") || diagnostic.contains("timeout"),
        "{diagnostic}"
    );
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
