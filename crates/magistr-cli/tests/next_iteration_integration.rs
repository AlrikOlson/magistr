//! One headless `magistr next` iteration, end to end, against a temporary
//! store — the tool sequence the `/next` skill prescribes, driven over the
//! same `magistr mcp-serve` surface an agent would call (in-memory duplex,
//! no LLM, no ACP agent). Proves:
//!
//! - the lifecycle guard refuses `execute` until an objective and a task are
//!   recorded in the ship engine, and refuses `done` until a check is
//!   recorded — no plan file is consulted anywhere;
//! - the four families end the iteration cross-referenced:
//!   `chunk:X` → `task:X` (roadmap ↔ ship), `think:N` → `task:X`
//!   (think ↔ ship) and `chunk:X` → `think:N` (roadmap ↔ think), plus a
//!   signal record, all persisted under the temp data dir.

use std::path::Path;

use magistr::mcp_tools::MagistrService;
use magistr_engine::native::NativeConfig;
use rmcp::model::{CallToolRequestParams, CallToolResult};
use rmcp::{ClientHandler, ServiceExt};
use serde_json::{Value, json};

#[derive(Clone)]
struct TestClient;
impl ClientHandler for TestClient {}

type Client = rmcp::service::RunningService<rmcp::RoleClient, TestClient>;

const CHUNK: &str = "e2e-guard-chunk";
const TASK: &str = "e2e-implement";

fn write_fixture(root: &Path) {
    std::fs::create_dir_all(root.join(".magistr")).expect("mkdir .magistr");
    std::fs::create_dir_all(root.join(".think-and-ship")).expect("mkdir identity");
    std::fs::write(
        root.join(".think-and-ship/project.json"),
        "{\"id\":\"next-iteration-fixture\"}\n",
    )
    .expect("identity");
    let config = json!({
        "project": {"name": "fixture", "description": "", "type": "cli"},
        "stack": {
            "language": "rust", "framework": "none", "package_manager": "cargo",
            "test_framework": "cargo-test", "linter": "clippy", "formatter": "rustfmt"
        },
        "quality_gate": {"check": "true", "test": "true", "lint": "true"},
        "format": {"command": "true", "extensions": ["rs"]},
        "spec": {"file": "ROADMAP.md", "unchecked_pattern": "^\\s*- \\[ \\]"},
        "testing": {"companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside"},
        "reference_repos": [],
        "state_prefix": "fixture",
        "roadmap_backend": "think-and-ship"
    });
    std::fs::write(
        root.join(".magistr/project.json"),
        serde_json::to_string_pretty(&config).expect("json"),
    )
    .expect("project.json");
}

async fn pair(root: &Path, data_dir: &Path) -> (Client, tokio::task::JoinHandle<()>) {
    let tas = NativeConfig::from_lookup(root, |_| None).with_data_dir(data_dir);
    let service = MagistrService::new(root.to_path_buf(), &tas)
        .await
        .expect("project owner");
    let (server_tx, client_tx) = tokio::io::duplex(1 << 16);
    let server = tokio::spawn(async move {
        let running = service.serve(server_tx).await.expect("server.serve");
        let _ = running.waiting().await;
    });
    let client = TestClient.serve(client_tx).await.expect("client.serve");
    (client, server)
}

async fn call(client: &Client, name: &str, args: Value) -> CallToolResult {
    let mut req = CallToolRequestParams::new(name.to_owned());
    req.arguments = args.as_object().cloned();
    client.peer().call_tool(req).await.expect("call_tool")
}

/// Call a tool that must succeed (no MCP error, no think-and-ship soft error).
async fn ok(client: &Client, name: &str, args: Value) -> Value {
    let result = call(client, name, args).await;
    assert_ne!(result.is_error, Some(true), "{name} errored: {result:?}");
    let body = text_json(&result);
    assert_ne!(body["ok"], false, "{name} soft-errored: {body}");
    body
}

fn text_json(result: &CallToolResult) -> Value {
    if let Some(v) = &result.structured_content {
        return v.clone();
    }
    let text = first_text(result);
    serde_json::from_str(&text).unwrap_or(Value::String(text))
}

fn first_text(result: &CallToolResult) -> String {
    result
        .content
        .iter()
        .find_map(|c| c.as_text().map(|t| t.text.clone()))
        .unwrap_or_default()
}

/// `magistr_set_phase` must refuse `phase` with a reason naming `expect`.
async fn phase_blocked(client: &Client, phase: &str, expect: &str) {
    let result = call(client, "magistr_set_phase", json!({"phase": phase})).await;
    assert_eq!(
        result.is_error,
        Some(true),
        "{phase} should be blocked: {result:?}"
    );
    let reason = first_text(&result);
    assert!(reason.contains(expect), "{phase} reason: {reason}");
    assert!(!reason.contains("current-plan.md"), "{reason}");
}

async fn phase_ok(client: &Client, phase: &str) {
    let result = call(client, "magistr_set_phase", json!({"phase": phase})).await;
    assert_ne!(
        result.is_error,
        Some(true),
        "{phase} should pass: {result:?}"
    );
}

fn has_session_file(data_dir: &Path, family: &str) -> bool {
    std::fs::read_dir(data_dir.join(family).join("sessions"))
        .map(|entries| {
            entries
                .flatten()
                .any(|e| e.path().extension().is_some_and(|x| x == "json"))
        })
        .unwrap_or(false)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn headless_iteration_is_guarded_on_ship_state_and_cross_referenced() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("proj");
    std::fs::create_dir_all(&root).expect("root");
    write_fixture(&root);
    let data_dir = tmp.path().join("data");
    let (client, server) = pair(&root, &data_dir).await;

    // ── Fixture roadmap: one ready chunk ────────────────────────────────
    ok(
        &client,
        "roadmap_add_chunk",
        json!({
            "id": CHUNK,
            "title": "Prove the guarded iteration end to end",
            "status": "pending",
            "priority": 100,
            "description": "fixture chunk for the ts2c e2e test",
            "acceptance": ["guard blocks, then passes", "families cross-referenced"],
            "group": "Fixture"
        }),
    )
    .await;
    let next = ok(&client, "roadmap_next", json!({})).await;
    assert_eq!(next["id"], CHUNK, "{next}");
    let started = ok(&client, "roadmap_start_chunk", json!({"id": CHUNK})).await;
    assert_eq!(started["chunk"]["status"], "in_progress", "{started}");
    assert_eq!(started["backref"], format!("chunk:{CHUNK}"));

    // ── Research and plan are always open; execute is not ──────────────
    phase_ok(&client, "research").await;
    phase_ok(&client, "plan").await;
    phase_blocked(&client, "execute", "ship_set_objective").await;

    // An objective alone is not a plan.
    ok(
        &client,
        "ship_set_objective",
        json!({
            "description": "realize the fixture chunk",
            "scope": format!("chunk:{CHUNK}"),
            "acceptance_criteria": ["the test passes"]
        }),
    )
    .await;
    phase_blocked(&client, "execute", "ship_plan").await;

    // Objective + one planned task: the guard opens.
    ok(
        &client,
        "ship_plan",
        json!({"action": "add", "task_id": TASK, "title": "implement", "task_type": "implement"}),
    )
    .await;
    phase_ok(&client, "execute").await;

    // ── Think open, cross-referenced to the task ───────────────────────
    let open = ok(
        &client,
        "think_record_step",
        json!({
            "purpose": "open the fixture iteration",
            "context": "e2e",
            "thought": "implement the one task",
            "outcome": "plan accepted",
            "next_action": "ship_start",
            "rationale": "one task, one commit",
            "estimated_total": 2,
            "execution_ref": format!("task:{TASK}")
        }),
    )
    .await;
    let step = open["step_number"].as_u64().expect("step_number");
    ok(
        &client,
        "roadmap_link",
        json!({"id": CHUNK, "cross_ref": format!("think:{step}")}),
    )
    .await;

    // ── Execute: start the task, record an action pointing back at the step
    ok(&client, "ship_start", json!({"task_id": TASK})).await;
    ok(
        &client,
        "ship_record",
        json!({
            "type": "code",
            "description": "edit the fixture source",
            "files_touched": ["src/lib.rs"],
            "think_step": step
        }),
    )
    .await;

    // ── Done needs a recorded check; verify is where the gates run ─────
    phase_blocked(&client, "done", "ship_check").await;
    phase_ok(&client, "verify").await;
    let gates = ok(&client, "magistr_gate", json!({"gate": "all"})).await;
    assert_eq!(gates["passed"], true, "{gates}");
    let rows = gates["gates"].as_array().expect("gates");
    assert_eq!(rows.len(), 3);
    assert!(
        rows.iter()
            .all(|g| g["recorded"] == true && g["verified"] == true),
        "{rows:?}"
    );

    // ── Close: task, chunk (with proof of ship), objective, signal, think ──
    ok(&client, "ship_complete", json!({"task_id": TASK})).await;
    let done = ok(
        &client,
        "roadmap_complete_chunk",
        json!({"id": CHUNK, "ship_ref": format!("task:{TASK}")}),
    )
    .await;
    assert_eq!(done["status"], "done", "{done}");
    let finalized = ok(
        &client,
        "ship_finalize",
        json!({"summary": "fixture shipped"}),
    )
    .await;
    assert_eq!(finalized["status"], "shipped", "{finalized}");
    assert_eq!(finalized["tasks"]["completed"], 1, "{finalized}");
    assert!(
        finalized["warnings"].as_array().is_some_and(Vec::is_empty),
        "no unverified or failed required checks: {finalized}"
    );
    ok(
        &client,
        "signal_capture",
        json!({"kind": "idea", "from": "e2e", "body": "discovered while shipping"}),
    )
    .await;
    ok(
        &client,
        "think_record_step",
        json!({
            "purpose": "close the fixture iteration",
            "context": "e2e",
            "thought": "shipped as planned",
            "outcome": "chunk done",
            "next_action": "stop",
            "rationale": "acceptance met",
            "estimated_total": 2,
            "execution_ref": format!("task:{TASK}"),
            "is_final_step": true
        }),
    )
    .await;
    phase_ok(&client, "done").await;

    // ── Cross-refs in all four families ────────────────────────────────
    let chunk = ok(&client, "roadmap_get", json!({"ids": [CHUNK]})).await;
    let record = &chunk["records"][0];
    let refs: Vec<&str> = record["cross_refs"]
        .as_array()
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    assert!(refs.contains(&format!("task:{TASK}").as_str()), "{refs:?}");
    assert!(refs.contains(&format!("think:{step}").as_str()), "{refs:?}");
    assert_eq!(record["status"], "done");
    assert_eq!(ok(&client, "roadmap_next", json!({})).await, Value::Null);

    let ship = ok(&client, "ship_status", json!({})).await;
    assert_eq!(ship["objective"]["status"], "completed", "{ship}");
    assert_eq!(ship["objective"]["scope"], format!("chunk:{CHUNK}"));
    let think_refs = ship["think_refs"].as_array().expect("think_refs");
    assert!(
        think_refs
            .iter()
            .any(|r| r["task_id"] == TASK && r["value"] == step),
        "{think_refs:?}"
    );
    let checks = ship["checks"].as_array().expect("checks");
    assert_eq!(checks.len(), 3);
    assert!(
        checks
            .iter()
            .all(|c| c["task_id"] == TASK && c["verified"] == true)
    );

    let think = ok(&client, "think_get_step", json!({"step_number": step})).await;
    assert_eq!(think["execution_ref"], format!("task:{TASK}"), "{think}");

    let signals = ok(&client, "signal_status", json!({})).await;
    assert_ne!(signals, Value::Null);

    // ── Persisted under the temp store, one file per family ────────────
    for family in ["roadmap", "ship", "think", "signal"] {
        assert!(
            has_session_file(&data_dir, family),
            "{family} session file missing"
        );
    }
    assert!(!root.join(".magistr/current-plan.md").exists());

    let _ = client.cancel().await;
    server.abort();
}
