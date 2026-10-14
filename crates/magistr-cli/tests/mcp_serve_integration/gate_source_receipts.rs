use super::*;
use magistr_workspace::snapshot::{SelectionPolicy, SourceSnapshot};

async fn begin_receipt_task(client: &Client) {
    call(
        client,
        "roadmap_add_chunk",
        json!({"id": "receipts", "title": "Verify current source"}),
    )
    .await;
    call(client, "roadmap_start_chunk", json!({"id": "receipts"})).await;
    call(
        client,
        "ship_set_objective",
        json!({
            "description": "Verify current source",
            "scope": "chunk:receipts",
            "verification": "focused"
        }),
    )
    .await;
    call(
        client,
        "ship_plan",
        json!({"action": "add", "task_id": "verify-source", "title": "Verify", "task_type": "test"}),
    )
    .await;
    let started = text_json(&call(client, "ship_start", json!({"task_id": "verify-source"})).await);
    assert_eq!(started["status"], "active", "{started}");
}

fn set_source_command(root: &Path, command: &str) {
    let path = root.join(".magistr/project.json");
    let mut config: Value =
        serde_json::from_slice(&std::fs::read(&path).expect("config")).expect("json");
    config["custom_gates"][0]["command"] = json!(command);
    std::fs::write(path, serde_json::to_vec(&config).expect("json")).expect("gate command");
}

async fn named_gate(client: &Client) -> Value {
    let gate = text_json(&call(client, "magistr_gate", json!({"gate": "structure"})).await);
    assert_eq!(gate["passed"], true, "{gate}");
    assert_eq!(gate["recorded"], true, "{gate}");
    assert_eq!(gate["source_freshness"]["freshness"], "unchanged", "{gate}");
    gate
}

async fn gate_receipt(client: &Client, gate: &Value) -> Value {
    let context = text_json(
        &call(
            client,
            "magistr_context",
            json!({"receipt_id": gate["receipt"]}),
        )
        .await,
    );
    context["receipt"].clone()
}

async fn complete_receipts(client: &Client) -> CallToolResult {
    call(
        client,
        "roadmap_complete_chunk",
        json!({"id": "receipts", "ship_ref": "task:verify-source"}),
    )
    .await
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn named_gates_and_ship_checks_run_in_the_observed_project_and_read_its_report() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("project");
    std::fs::create_dir_all(root.join("results")).expect("root");
    write_green_fixture(&root);
    std::fs::write(root.join(".gitignore"), "/results/\n").expect("ignored reports");
    std::fs::write(root.join("gate-source.txt"), "project source\n").expect("source");
    let command = "cat gate-source.txt && printf '%s' '<testsuite tests=\"1\" failures=\"0\" errors=\"0\"><testcase name=\"source\"/></testsuite>' > results/junit.xml";
    set_source_command(&root, command);
    let (client, server) = pair(&root, &tmp.path().join("data")).await;
    begin_receipt_task(&client).await;

    let named = named_gate(&client).await;
    assert!(
        named["output"]
            .as_str()
            .expect("output")
            .contains("project source")
    );
    let current = SourceSnapshot::capture(&root, SelectionPolicy::default()).expect("source");
    assert_eq!(named["source_revision"], current.revision());

    let direct = text_json(
        &call(
            &client,
            "ship_check",
            json!({
                "type": "test", "name": "direct-source", "command": command,
                "report": {"format": "junit", "path": "results/junit.xml"}
            }),
        )
        .await,
    );
    assert_eq!(direct["passed"], true, "{direct}");
    assert_eq!(direct["verified"], true, "{direct}");
    assert!(
        direct["details"]
            .as_str()
            .expect("details")
            .contains("project source")
    );
    assert_eq!(direct["report"]["parsed"], true, "{direct}");
    assert_eq!(direct["results"]["passed"], 1, "{direct}");
    let completed = complete_receipts(&client).await;
    assert_ne!(completed.is_error, Some(true), "{completed:?}");

    client.cancel().await.expect("cancel client");
    server.await.expect("server stopped");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn named_gate_reruns_capture_source_and_evidence_edits_and_completion_stays_strict() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("project");
    std::fs::create_dir_all(&root).expect("root");
    write_green_fixture(&root);
    std::fs::write(root.join("source.txt"), "before\n").expect("source");
    let (client, server) = pair(&root, &tmp.path().join("data")).await;
    begin_receipt_task(&client).await;

    let first = named_gate(&client).await;
    let first_receipt = gate_receipt(&client, &first).await;
    assert_eq!(first_receipt["attempt"], 1);
    assert_eq!(
        first_receipt["source"]["revision"],
        first["source_revision"]
    );

    std::fs::write(root.join("source.txt"), "after\n").expect("source edit");
    std::fs::write(root.join("evidence.txt"), "final evidence\n").expect("evidence edit");
    let stale = complete_receipts(&client).await;
    assert_eq!(stale.is_error, Some(true), "{stale:?}");
    assert!(
        text_json(&stale)["message"]
            .as_str()
            .expect("reason")
            .contains("source changed")
    );

    let second = named_gate(&client).await;
    let second_receipt = gate_receipt(&client, &second).await;
    assert_ne!(second["receipt"], first["receipt"]);
    assert_ne!(second["source_revision"], first["source_revision"]);
    assert_eq!(second_receipt["attempt"], 2);
    assert_eq!(second_receipt["target"], first_receipt["target"]);
    let current = SourceSnapshot::capture(&root, SelectionPolicy::default()).expect("final source");
    assert_eq!(second_receipt["source"]["revision"], current.revision());

    let repeated = named_gate(&client).await;
    assert_ne!(repeated["receipt"], second["receipt"]);
    assert_eq!(repeated["source_revision"], second["source_revision"]);
    assert_eq!(gate_receipt(&client, &repeated).await["attempt"], 3);
    let completed = complete_receipts(&client).await;
    assert_ne!(completed.is_error, Some(true), "{completed:?}");
    assert_eq!(text_json(&completed)["status"], "done");

    std::fs::write(root.join("source.txt"), "later\n").expect("later source edit");
    let stale = complete_receipts(&client).await;
    assert_eq!(stale.is_error, Some(true), "{stale:?}");
    assert!(
        text_json(&stale)["message"]
            .as_str()
            .expect("reason")
            .contains("source changed")
    );

    client.cancel().await.expect("cancel client");
    server.await.expect("server stopped");
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_named_gate_records_source_changes_during_execution_and_requires_a_rerun() {
    use std::time::Duration;

    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("project");
    std::fs::create_dir_all(&root).expect("root");
    write_green_fixture(&root);
    std::fs::write(root.join(".gitignore"), "/gate-started\n/gate-release\n").expect("barriers");
    std::fs::write(root.join("source.txt"), "before\n").expect("source");
    set_source_command(
        &root,
        "touch gate-started; while [ ! -f gate-release ]; do sleep 0.01; done",
    );
    let (client, server) = pair(&root, &tmp.path().join("data")).await;
    begin_receipt_task(&client).await;

    let running = call(&client, "magistr_gate", json!({"gate": "structure"}));
    let edit = async {
        tokio::time::timeout(Duration::from_secs(5), async {
            while !root.join("gate-started").exists() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("gate starts");
        std::fs::write(root.join("source.txt"), "during\n").expect("source edit");
        std::fs::write(root.join("gate-release"), "").expect("release gate");
    };
    let (gate, ()) = tokio::join!(running, edit);
    let gate = text_json(&gate);
    assert_eq!(gate["passed"], true, "{gate}");
    assert_eq!(gate["recorded"], true, "{gate}");
    assert_eq!(gate["source_freshness"]["freshness"], "changed", "{gate}");
    let receipt = gate_receipt(&client, &gate).await;
    assert_eq!(
        receipt["freshness"]["paths"],
        json!(["source.txt (Modified)"])
    );
    let stale = complete_receipts(&client).await;
    assert_eq!(stale.is_error, Some(true), "{stale:?}");
    assert!(
        text_json(&stale)["message"]
            .as_str()
            .expect("reason")
            .contains("changed while the gate ran")
    );

    let rerun = named_gate(&client).await;
    assert_ne!(rerun["source_revision"], gate["source_revision"]);
    let completed = complete_receipts(&client).await;
    assert_ne!(completed.is_error, Some(true), "{completed:?}");

    client.cancel().await.expect("cancel client");
    server.await.expect("server stopped");
}
