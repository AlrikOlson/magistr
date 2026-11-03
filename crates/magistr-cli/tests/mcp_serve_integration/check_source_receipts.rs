use super::*;
use magistr_workspace::snapshot::{SelectionPolicy, SourceSnapshot};

async fn begin_receipt_task(client: &Client) {
    call(
        client,
        "plan_add_task",
        json!({"id": "receipts", "title": "Verify current source"}),
    )
    .await;
    call(client, "plan_start_task", json!({"id": "receipts"})).await;
    call(
        client,
        "run_open",
        json!({
            "description": "Verify current source",
            "scope": "task:receipts",
            "verification": "focused"
        }),
    )
    .await;
    call(
        client,
        "arrange_steps",
        json!({"action": "add", "step_id": "verify-source", "title": "Verify", "step_type": "test"}),
    )
    .await;
    let started = text_json(&call(client, "start_step", json!({"step_id": "verify-source"})).await);
    assert_eq!(started["value"]["status"], "active", "{started}");
}

#[cfg(unix)]
fn set_source_command(root: &Path, command: &str) {
    let path = root.join(".magistr/project.json");
    let mut config: Value =
        serde_json::from_slice(&std::fs::read(&path).expect("config")).expect("json");
    config["custom_checks"][0]["command"] = json!(command);
    std::fs::write(path, serde_json::to_vec(&config).expect("json")).expect("check command");
}

async fn named_check(client: &Client) -> Value {
    let check = text_json(&call(client, "magistr_check", json!({"check": "structure"})).await);
    assert_eq!(check["passed"], true, "{check}");
    assert_eq!(check["recorded"], true, "{check}");
    assert_eq!(
        check["source_freshness"]["freshness"], "unchanged",
        "{check}"
    );
    check
}

async fn check_receipt(client: &Client, check: &Value) -> Value {
    let context = text_json(
        &call(
            client,
            "magistr_context",
            json!({"check_result_id": check["check_result"]}),
        )
        .await,
    );
    context["check_result"].clone()
}

async fn complete_receipts(client: &Client) -> CallToolResult {
    call(
        client,
        "plan_complete_task",
        json!({"id": "receipts", "step_ref": "step:verify-source"}),
    )
    .await
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn named_checks_and_run_checks_run_in_the_observed_project_and_read_its_report() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("project");
    std::fs::create_dir_all(root.join("results")).expect("root");
    write_green_fixture(&root);
    std::fs::write(root.join(".gitignore"), "/results/\n").expect("ignored reports");
    std::fs::write(root.join("check-source.txt"), "project source\n").expect("source");
    let command = "cat check-source.txt && printf '%s' '<testsuite tests=\"1\" failures=\"0\" errors=\"0\"><testcase name=\"source\"/></testsuite>' > results/junit.xml";
    set_source_command(&root, command);
    let (client, server) = pair(&root, &tmp.path().join("data")).await;
    begin_receipt_task(&client).await;

    let named = named_check(&client).await;
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
            "check_step",
            json!({
                "type": "test", "name": "direct-source", "command": command,
                "report": {"format": "junit", "path": "results/junit.xml"}
            }),
        )
        .await,
    );
    assert_eq!(direct["settlement"], "applied", "{direct}");
    let direct = &direct["check"]["value"];
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
async fn named_check_reruns_capture_source_and_evidence_edits_and_completion_stays_strict() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("project");
    std::fs::create_dir_all(&root).expect("root");
    write_green_fixture(&root);
    std::fs::write(root.join("source.txt"), "before\n").expect("source");
    let (client, server) = pair(&root, &tmp.path().join("data")).await;
    begin_receipt_task(&client).await;

    let first = named_check(&client).await;
    let first_receipt = check_receipt(&client, &first).await;
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

    let second = named_check(&client).await;
    let second_receipt = check_receipt(&client, &second).await;
    assert_ne!(second["check_result"], first["check_result"]);
    assert_ne!(second["source_revision"], first["source_revision"]);
    assert_eq!(second_receipt["attempt"], 2);
    assert_eq!(second_receipt["target"], first_receipt["target"]);
    let current = SourceSnapshot::capture(&root, SelectionPolicy::default()).expect("final source");
    assert_eq!(second_receipt["source"]["revision"], current.revision());

    let repeated = named_check(&client).await;
    assert_ne!(repeated["check_result"], second["check_result"]);
    assert_eq!(repeated["source_revision"], second["source_revision"]);
    assert_eq!(check_receipt(&client, &repeated).await["attempt"], 3);
    let completed = complete_receipts(&client).await;
    assert_ne!(completed.is_error, Some(true), "{completed:?}");
    assert_eq!(text_json(&completed)["value"]["status"], "done");

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
async fn a_named_check_records_source_changes_during_execution_and_requires_a_rerun() {
    use std::time::Duration;

    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("project");
    std::fs::create_dir_all(&root).expect("root");
    write_green_fixture(&root);
    std::fs::write(root.join(".gitignore"), "/check-started\n/check-release\n").expect("barriers");
    std::fs::write(root.join("source.txt"), "before\n").expect("source");
    set_source_command(
        &root,
        "touch check-started; while [ ! -f check-release ]; do sleep 0.01; done",
    );
    let (client, server) = pair(&root, &tmp.path().join("data")).await;
    begin_receipt_task(&client).await;

    let running = call(&client, "magistr_check", json!({"check": "structure"}));
    let edit = async {
        tokio::time::timeout(Duration::from_secs(5), async {
            while !root.join("check-started").exists() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("check starts");
        std::fs::write(root.join("source.txt"), "during\n").expect("source edit");
        std::fs::write(root.join("check-release"), "").expect("release check");
    };
    let (check, ()) = tokio::join!(running, edit);
    let check = text_json(&check);
    assert_eq!(check["passed"], true, "{check}");
    assert_eq!(check["recorded"], true, "{check}");
    assert_eq!(check["source_freshness"]["freshness"], "changed", "{check}");
    let receipt = check_receipt(&client, &check).await;
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
            .contains("changed while the check ran")
    );

    let rerun = named_check(&client).await;
    assert_ne!(rerun["source_revision"], check["source_revision"]);
    let completed = complete_receipts(&client).await;
    assert_ne!(completed.is_error, Some(true), "{completed:?}");

    client.cancel().await.expect("cancel client");
    server.await.expect("server stopped");
}
