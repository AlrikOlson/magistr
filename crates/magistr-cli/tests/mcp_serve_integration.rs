//! `magistr mcp-serve` over a real rmcp client on an in-memory duplex:
//! one tool list carrying both the `magistr_*` tools and the think-and-ship
//! families, gates recorded through `ship_check`, phases recorded on the
//! active task — all against a temporary store.

use std::path::Path;

use magistr::mcp_tools::MagistrService;
use rmcp::model::{CallToolRequestParams, CallToolResult};
use rmcp::{ClientHandler, ServiceExt};
use serde_json::{Value, json};

#[derive(Clone)]
struct TestClient;
impl ClientHandler for TestClient {}

type Client = rmcp::service::RunningService<rmcp::RoleClient, TestClient>;

/// The gate commands are written for two shells at once: magistr runs a
/// gate through `sh -c`, and think-and-ship's `ship_check` (which runs a
/// recorded gate itself) uses `cmd /C` on Windows, where any quote in
/// the command is mangled on the way in. So no quotes: a diagnostic that
/// needs them lives in `diag.sh` and the gate is `sh <root>/diag.sh` —
/// by absolute path, since `ship_check` runs in the server's own cwd.
fn write_fixture(root: &Path) {
    std::fs::create_dir_all(root.join(".magistr")).expect("mkdir .magistr");
    std::fs::write(
        root.join("diag.sh"),
        "printf 'error[E0308]: mismatched types\\n --> src/lib.rs:3:5\\n'\nexit 1\n",
    )
    .expect("diag.sh");
    std::fs::create_dir_all(root.join(".think-and-ship")).expect("mkdir identity");
    std::fs::write(
        root.join(".think-and-ship/project.json"),
        "{\"id\":\"mcp-serve-fixture\"}\n",
    )
    .expect("identity");
    let config = json!({
        "project": {"name": "fixture", "description": "", "type": "cli"},
        "stack": {
            "language": "rust", "framework": "none", "package_manager": "cargo",
            "test_framework": "cargo-test", "linter": "clippy", "formatter": "rustfmt"
        },
        "quality_gate": {"check": "true", "test": "true", "lint": "exit 3"},
        "custom_gates": [{
            "name": "diag",
            "command": format!("sh {}/diag.sh", root.to_string_lossy().replace('\\', "/")),
            "required": false
        }],
        "format": {"command": "true", "extensions": ["rs"]},
        "spec": {"file": "ROADMAP.md", "unchecked_pattern": "^\\s*- \\[ \\]"},
        "testing": {"companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside"},
        "reference_repos": [],
        "state_prefix": "fixture"
    });
    std::fs::write(
        root.join(".magistr/project.json"),
        serde_json::to_string_pretty(&config).expect("json"),
    )
    .expect("project.json");
}

async fn pair(root: &Path, data_dir: &Path) -> (Client, tokio::task::JoinHandle<()>) {
    let tas = magistr_core::evidence::writer_config(root, |_| None).with_data_dir(data_dir);
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

fn text_json(result: &CallToolResult) -> Value {
    if let Some(v) = &result.structured_content {
        return v.clone();
    }
    let text = result
        .content
        .iter()
        .find_map(|c| c.as_text().map(|t| t.text.clone()))
        .expect("text content");
    serde_json::from_str(&text).expect("json text")
}

#[cfg(unix)]
async fn check_keeps_its_admitted_run(tool: &str) {
    use std::time::Duration;
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("project");
    std::fs::create_dir_all(&root).expect("root");
    write_fixture(&root);
    let started = root.join("started");
    let release = root.join("release");
    let script = root.join("waiting.sh");
    std::fs::write(
        &script,
        format!(
            "touch '{}'; while [ ! -f '{}' ]; do sleep 0.01; done; printf 'finished\\n'\n",
            started.display(),
            release.display(),
        ),
    )
    .expect("barrier command");
    let command = format!("sh '{}'", script.display());
    let config_path = root.join(".magistr/project.json");
    let mut config: Value =
        serde_json::from_slice(&std::fs::read(&config_path).expect("config")).expect("json");
    config["quality_gate"]["check"] = json!(command);
    std::fs::write(config_path, serde_json::to_vec(&config).expect("json")).expect("config");
    let (client, server) = pair(&root, &tmp.path().join("data")).await;
    begin_reused_task(&client, "first").await;
    let args = if tool == "magistr_gate" {
        json!({"gate": "check"})
    } else {
        json!({"type": "test", "name": "slow", "command": command})
    };
    let running = call(&client, tool, args);
    let rollover = async {
        tokio::time::timeout(Duration::from_secs(5), async {
            while !started.exists() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            call(&client, "ship_complete", json!({"task_id": "reused"})).await;
            call(&client, "ship_finalize", json!({})).await;
            begin_reused_task(&client, "second").await;
        })
        .await
        .expect("workflow stays responsive while gate waits");
        std::fs::write(&release, "").expect("release command");
    };
    let (result, ()) = tokio::join!(running, rollover);
    let result = text_json(&result);
    if tool == "magistr_gate" {
        assert_eq!(result["passed"], true, "{result}");
        assert_eq!(result["recorded"], false, "{result}");
        assert_eq!(result["stale_admission"]["target"]["task_id"], "reused");
        assert_eq!(result["unsettled_outcome"]["exit_code"], 0);
    } else {
        assert_eq!(result["error_kind"], "stale_result", "{result}");
        assert_eq!(result["admission"]["target"]["task_id"], "reused");
        assert_eq!(result["outcome"]["exit_code"], 0);
    }
    let status = text_json(&call(&client, "ship_status", json!({})).await);
    assert_eq!(status["objective"]["description"], "second", "{status}");
    assert!(
        status["checks"].as_array().expect("checks").is_empty(),
        "{status}"
    );
    client.cancel().await.expect("cancel client");
    server.await.expect("server stopped");
}

#[cfg(unix)]
async fn begin_reused_task(client: &Client, objective: &str) {
    call(
        client,
        "ship_set_objective",
        json!({"description": objective}),
    )
    .await;
    call(
        client,
        "ship_plan",
        json!({"action": "add", "task_id": "reused", "title": "Verify"}),
    )
    .await;
    let started = call(client, "ship_start", json!({"task_id": "reused"})).await;
    assert_eq!(text_json(&started)["status"], "active", "{started:?}");
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn magistr_gate_cannot_land_on_a_reused_task_in_a_later_run() {
    check_keeps_its_admitted_run("magistr_gate").await;
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_ship_check_cannot_land_on_a_reused_task_in_a_later_run() {
    check_keeps_its_admitted_run("ship_check").await;
}

#[tokio::test]
async fn roadmap_export_writes_live_state_in_the_project_and_returns_only_a_receipt() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("project");
    std::fs::create_dir_all(&root).expect("root");
    write_fixture(&root);
    let path = root.join("ROADMAP.md");
    std::fs::write(&path, "stale projection").expect("old export");
    let (client, server) = pair(&root, &tmp.path().join("data")).await;
    call(
        &client,
        "roadmap_add_chunk",
        json!({
            "id": "export-test", "title": "Write the generated roadmap", "group": "Exports",
            "description": "large body ".repeat(2000)
        }),
    )
    .await;

    let tools = client.peer().list_tools(None).await.expect("tools");
    let tool = tools
        .tools
        .iter()
        .find(|t| t.name == "roadmap_export")
        .expect("export tool");
    assert_eq!(tool.input_schema["properties"]["output"]["default"], "file");
    assert_eq!(
        tool.annotations.as_ref().and_then(|a| a.read_only_hint),
        Some(false)
    );
    let response = call(&client, "roadmap_export", json!({})).await;
    let receipt = text_json(&response);
    assert_eq!(receipt["written"], true, "{receipt}");
    assert_eq!(
        receipt["path"],
        path.canonicalize()
            .expect("absolute export")
            .to_string_lossy()
            .as_ref()
    );
    assert!(
        receipt.get("roadmap").is_none(),
        "no document in the receipt"
    );
    assert!(serde_json::to_vec(&response).expect("json").len() < 1024);
    let contents = std::fs::read_to_string(&path).expect("exported roadmap");
    assert!(contents.contains("Write the generated roadmap"));
    assert!(contents.len() > 20_000);
    assert_eq!(receipt["bytes"], contents.len());

    call(
        &client,
        "roadmap_update_chunk",
        json!({
            "id": "export-test", "title": "Export the latest in-process state"
        }),
    )
    .await;
    call(&client, "roadmap_export", json!({})).await;
    assert!(
        std::fs::read_to_string(&path)
            .expect("new export")
            .contains("Export the latest in-process state")
    );
    client.cancel().await.expect("cancel");
    server.await.expect("server");
}

#[tokio::test]
async fn roadmap_export_inline_is_explicit_and_json_writes_its_own_file() {
    let tmp = tempfile::tempdir().expect("tempdir");
    write_fixture(tmp.path());
    let (client, server) = pair(tmp.path(), &tmp.path().join("data")).await;
    let inline = text_json(&call(&client, "roadmap_export", json!({"output": "inline"})).await);
    assert!(inline["roadmap"].is_string(), "{inline}");
    assert!(!tmp.path().join("ROADMAP.md").exists());
    let receipt = text_json(&call(&client, "roadmap_export", json!({"format": "json"})).await);
    assert_eq!(receipt["written"], true, "{receipt}");
    assert_eq!(receipt["format"], "json");
    let json = std::fs::read_to_string(tmp.path().join("ROADMAP.json")).expect("json export");
    serde_json::from_str::<Value>(&json).expect("valid JSON");
    assert_eq!(receipt["bytes"], json.len());
    assert!(!tmp.path().join("ROADMAP.md").exists());
    client.cancel().await.expect("cancel");
    server.await.expect("server");
}

#[tokio::test]
async fn roadmap_export_policy_updates_apply_to_running_server_without_blocking_native_reads() {
    let tmp = tempfile::tempdir().expect("project");
    write_fixture(tmp.path());
    let (client, server) = pair(tmp.path(), &tmp.path().join("data")).await;
    let config_path = tmp.path().join(".magistr/project.json");
    let mut config: Value =
        serde_json::from_str(&std::fs::read_to_string(&config_path).expect("config"))
            .expect("json");
    config["roadmap_exports"] = json!(false);
    std::fs::write(&config_path, config.to_string()).expect("disable exports after startup");
    for args in [
        json!({}),
        json!({"format":"markdown","output":"file"}),
        json!({"format":"json"}),
    ] {
        let result = text_json(&call(&client, "roadmap_export", args).await);
        assert_eq!(result["ok"], false, "{result}");
        assert_eq!(result["error_kind"], "export_disabled", "{result}");
        assert!(
            result.get("roadmap").is_none(),
            "no document payload: {result}"
        );
        assert!(!tmp.path().join("ROADMAP.md").exists());
        assert!(!tmp.path().join("ROADMAP.json").exists());
    }
    let result = text_json(&call(&client, "roadmap_export", json!({"output":"inline"})).await);
    assert!(result["roadmap"].is_string(), "{result}");
    assert!(!tmp.path().join("ROADMAP.md").exists());
    let result = text_json(&call(&client, "roadmap_status", json!({})).await);
    assert_ne!(result["ok"], false, "{result}");
    client.cancel().await.expect("cancel");
    server.await.expect("server");
}

#[tokio::test]
async fn roadmap_export_refuses_bad_arguments_and_non_file_targets_without_partial_writes() {
    let tmp = tempfile::tempdir().expect("tempdir");
    write_fixture(tmp.path());
    let target = tmp.path().join("ROADMAP.md");
    std::fs::create_dir(&target).expect("directory target");
    let (client, server) = pair(tmp.path(), &tmp.path().join("data")).await;
    for args in [
        json!({"format": "html"}),
        json!({"output": "stdout"}),
        json!({"format": 42}),
        json!({"output": null}),
    ] {
        let error = text_json(&call(&client, "roadmap_export", args).await);
        assert_eq!(error["ok"], false, "{error}");
        assert_eq!(error["error_kind"], "invalid_args", "{error}");
    }
    let error = text_json(&call(&client, "roadmap_export", json!({})).await);
    assert_eq!(error["error_kind"], "export_failed", "{error}");
    assert!(target.is_dir());
    assert!(
        !std::fs::read_dir(tmp.path())
            .expect("root entries")
            .any(|entry| entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .starts_with(".tmp"))
    );
    client.cancel().await.expect("cancel");
    server.await.expect("server");
}

#[cfg(unix)]
#[tokio::test]
async fn roadmap_export_never_follows_a_destination_symlink() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("project");
    std::fs::create_dir(&root).expect("root");
    write_fixture(&root);
    let outside = tmp.path().join("outside.md");
    std::fs::write(&outside, "keep me").expect("outside");
    std::os::unix::fs::symlink(&outside, root.join("ROADMAP.md")).expect("symlink");
    let (client, server) = pair(&root, &tmp.path().join("data")).await;
    let error = text_json(&call(&client, "roadmap_export", json!({})).await);
    assert_eq!(error["error_kind"], "export_failed", "{error}");
    assert_eq!(
        std::fs::read_to_string(outside).expect("outside"),
        "keep me"
    );
    assert!(root.join("ROADMAP.md").is_symlink());
    client.cancel().await.expect("cancel");
    server.await.expect("server");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn one_server_serves_both_surfaces_and_records_gates_through_ship() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("proj");
    std::fs::create_dir_all(&root).expect("root");
    write_fixture(&root);
    let data_dir = tmp.path().join("data");
    let (client, server) = pair(&root, &data_dir).await;

    // One tool list, both surfaces.
    let tools = client.peer().list_tools(None).await.expect("list_tools");
    let names: Vec<&str> = tools.tools.iter().map(|t| t.name.as_ref()).collect();
    for expected in [
        "magistr_gate",
        "magistr_set_phase",
        "magistr_format",
        "think_record_step",
        "ship_check",
        "roadmap_next",
        "roadmap_status",
        "signal_capture",
        "tracker_status",
    ] {
        assert!(
            names.contains(&expected),
            "{expected} missing from {names:?}"
        );
    }

    // A think-and-ship tool answers through the same server.
    let status = call(&client, "roadmap_status", json!({})).await;
    assert_ne!(status.is_error, Some(true));
    assert_eq!(text_json(&status)["project_id"], "mcp-serve-fixture");

    // Without an active task the gate still runs, and says it was not recorded.
    let unrecorded = call(&client, "magistr_gate", json!({"gate": "check"})).await;
    let body = text_json(&unrecorded);
    assert_eq!(body["passed"], true, "{body}");
    assert_eq!(body["recorded"], false, "{body}");

    // Objective → plan → start, then the gate is recorded through ship_check.
    call(
        &client,
        "ship_set_objective",
        json!({"description": "ts2 duplex"}),
    )
    .await;
    call(
        &client,
        "ship_plan",
        json!({"action": "add", "task_id": "gate-task", "title": "run gates"}),
    )
    .await;
    let started = call(&client, "ship_start", json!({"task_id": "gate-task"})).await;
    assert_ne!(started.is_error, Some(true), "{started:?}");

    let recorded = call(&client, "magistr_gate", json!({"gate": "all"})).await;
    let body = text_json(&recorded);
    assert_eq!(body["passed"], false, "lint exits 3: {body}");
    let gates = body["gates"].as_array().expect("gates");
    assert_eq!(
        gates.len(),
        3,
        "check, test, lint — `all` is standard + *required* custom gates, and \
         `diag` is registered not-required: {body}"
    );
    for gate in gates {
        assert_eq!(gate["recorded"], true, "{gate}");
        assert_eq!(gate["verified"], true, "{gate}");
        assert!(gate["rows"].is_array(), "every gate carries rows: {gate}");
    }
    let lint = gates
        .iter()
        .find(|g| g["gate"] == "lint")
        .expect("lint gate");
    assert_eq!(lint["exit_code"], 3);
    assert_eq!(lint["passed"], false);
    assert_eq!(
        lint["rows"].as_array().map(Vec::len),
        Some(0),
        "no parsable output"
    );

    // wb3: a gate whose output is a rustc-style diagnostic answers with rows.
    // Reached by its own name, which is where a not-required gate lives —
    // `all` no longer pays for a run whose red it would have to ignore.
    let by_name = call(&client, "magistr_gate", json!({"gate": "diag"})).await;
    let diag = text_json(&by_name);
    assert_eq!(diag["recorded"], true, "{diag}");
    assert_eq!(diag["verified"], true, "{diag}");
    assert_eq!(diag["passed"], false);
    assert_eq!(diag["required"], false);
    let rows = diag["rows"].as_array().expect("rows");
    assert_eq!(rows.len(), 1, "{diag}");
    assert_eq!(rows[0]["severity"], "error");
    assert_eq!(rows[0]["code"], "E0308");
    assert_eq!(rows[0]["message"], "mismatched types");
    assert_eq!(rows[0]["file"], "src/lib.rs");
    assert_eq!(rows[0]["line"], 3);
    assert_eq!(rows[0]["column"], 5);

    // The phase transition lands as an action on the active task — a
    // narrative record of the tool call; the phase itself travels typed on
    // the feed, and nothing reads it back out of these words.
    let phase = call(&client, "magistr_set_phase", json!({"phase": "verify"})).await;
    assert_ne!(phase.is_error, Some(true), "{phase:?}");

    let ship = text_json(&call(&client, "ship_status", json!({})).await);
    let task = ship["task_list"]
        .as_array()
        .and_then(|t| t.iter().find(|t| t["id"] == "gate-task"))
        .cloned()
        .unwrap_or(ship.clone());
    let checks = ship["checks"].as_array().cloned().unwrap_or_default();
    assert_eq!(checks.len(), 4, "four verified checks: {ship}");
    assert!(checks.iter().all(|c| c["verified"] == true), "{checks:?}");
    // ship_status summarises checks (name/passed/verified); the exit code was
    // asserted on the gate response above.
    assert!(
        checks
            .iter()
            .any(|c| c["name"] == "lint" && c["passed"] == false && c["verified"] == true),
        "{checks:?}"
    );
    let actions = ship["recent_actions"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        actions.iter().any(|a| a["tools_used"]
            .as_array()
            .is_some_and(|tools| tools.iter().any(|t| t == "magistr_set_phase"))
            && a["description"] == "workflow phase → Verifying"),
        "phase action recorded: {actions:?} (task {task})"
    );

    // Everything persisted under the temp store, keyed by the declared identity.
    assert!(
        data_dir
            .join("ship/sessions/mcp-serve-fixture.json")
            .exists()
    );

    let _ = client.cancel().await;
    server.abort();
}

/// The Claude Agent SDK's bundled CLI requests protocol 2026-07-28 and then
/// rejects the SEP-2322 `resultType` rmcp stamps on that era's results. The
/// capped transport must negotiate the session down to 2025-06-18 and serve
/// a `tools/list` with no `resultType` — the raw wire is what that CLI
/// validates, so this test speaks raw JSON lines rather than the rmcp client.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_modern_client_is_negotiated_down_and_tools_list_carries_no_result_type() {
    use magistr::mcp_tools::protocol_cap::ProtocolCap;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("proj");
    std::fs::create_dir_all(&root).expect("root");
    write_fixture(&root);
    let data_dir = tmp.path().join("data");

    let tas = magistr_core::evidence::writer_config(&root, |_| None).with_data_dir(&data_dir);
    let service = MagistrService::new(root.clone(), &tas)
        .await
        .expect("project owner");
    let (server_tx, client_tx) = tokio::io::duplex(1 << 16);
    let transport = ProtocolCap::new(
        &service,
        rmcp::transport::IntoTransport::<rmcp::service::RoleServer, std::io::Error, _>::into_transport(
            server_tx,
        ),
    );
    let server = tokio::spawn(async move {
        let running = service.serve(transport).await.expect("server.serve");
        let _ = running.waiting().await;
    });

    let (read, mut write) = tokio::io::split(client_tx);
    let mut lines = BufReader::new(read).lines();
    async fn ask(
        write: &mut tokio::io::WriteHalf<tokio::io::DuplexStream>,
        lines: &mut tokio::io::Lines<BufReader<tokio::io::ReadHalf<tokio::io::DuplexStream>>>,
        line: String,
    ) -> Value {
        write.write_all(line.as_bytes()).await.expect("write");
        write.write_all(b"\n").await.expect("newline");
        let reply = lines
            .next_line()
            .await
            .expect("read")
            .expect("a response line");
        serde_json::from_str(&reply).expect("response json")
    }

    let init = ask(
        &mut write,
        &mut lines,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {
                "protocolVersion": "2026-07-28",
                "capabilities": {},
                "clientInfo": {"name": "sdk-cli-probe", "version": "0"}
            }
        })
        .to_string(),
    )
    .await;
    assert_eq!(
        init["result"]["protocolVersion"], "2025-06-18",
        "negotiated down: {init}"
    );

    write
        .write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n")
        .await
        .expect("initialized");

    let tools = ask(
        &mut write,
        &mut lines,
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}).to_string(),
    )
    .await;
    let result = &tools["result"];
    assert!(
        result.get("resultType").is_none(),
        "no SEP-2322 discriminator on the wire: {result}"
    );
    let names: Vec<&str> = result["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    for expected in ["magistr_gate", "roadmap_next", "ship_check"] {
        assert!(names.contains(&expected), "{expected} in {names:?}");
    }

    // `server/discover` is the other place a client learns the ceiling — the
    // SDK CLI negotiates from it and must never see 2026-07-28 on offer.
    let discover = ask(
        &mut write,
        &mut lines,
        json!({"jsonrpc": "2.0", "id": 3, "method": "server/discover", "params": {"_meta": {
            "io.modelcontextprotocol/protocolVersion": "2025-06-18",
            "io.modelcontextprotocol/clientInfo": {"name": "sdk-cli-probe", "version": "0"},
            "io.modelcontextprotocol/clientCapabilities": {}
        }}})
        .to_string(),
    )
    .await;
    let advertised = serde_json::to_string(&discover["result"]).expect("discover json");
    assert!(
        !advertised.contains("2026-07-28"),
        "no modern era on offer: {advertised}"
    );
    assert!(advertised.contains("2025-06-18"), "{advertised}");

    server.abort();
}

/// Claude Code opens with `server/discover`, is offered only the capped
/// eras, and falls back to `initialize`. rmcp holds a session whose first
/// message was not `initialize` to per-request `_meta` for good, so the cap
/// answers that opening discover itself; the legacy requests that follow
/// must be served.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_client_that_discovers_before_initialize_still_lists_tools() {
    use magistr::mcp_tools::protocol_cap::ProtocolCap;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("proj");
    std::fs::create_dir_all(&root).expect("root");
    write_fixture(&root);
    let data_dir = tmp.path().join("data");

    let tas = magistr_core::evidence::writer_config(&root, |_| None).with_data_dir(&data_dir);
    let service = MagistrService::new(root.clone(), &tas)
        .await
        .expect("project owner");
    let (server_tx, client_tx) = tokio::io::duplex(1 << 16);
    let transport = ProtocolCap::new(
        &service,
        rmcp::transport::IntoTransport::<rmcp::service::RoleServer, std::io::Error, _>::into_transport(
            server_tx,
        ),
    );
    let server = tokio::spawn(async move {
        let running = service.serve(transport).await.expect("server.serve");
        let _ = running.waiting().await;
    });

    let (read, mut write) = tokio::io::split(client_tx);
    let mut lines = BufReader::new(read).lines();
    let mut replies = Vec::new();
    for request in [
        json!({"jsonrpc": "2.0", "id": 0, "method": "server/discover", "params": {"_meta": {
            "io.modelcontextprotocol/protocolVersion": "2026-07-28",
            "io.modelcontextprotocol/clientInfo": {"name": "discover-first", "version": "0"},
            "io.modelcontextprotocol/clientCapabilities": {}
        }}}),
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "discover-first", "version": "0"}
        }}),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
        json!({"jsonrpc": "2.0", "id": 3, "method": "ping"}),
    ] {
        write
            .write_all(
                format!(
                    "{request}
"
                )
                .as_bytes(),
            )
            .await
            .expect("write");
        let reply = lines
            .next_line()
            .await
            .expect("read")
            .expect("a response line");
        replies.push(serde_json::from_str::<Value>(&reply).expect("response json"));
    }

    let advertised = serde_json::to_string(&replies[0]["result"]).expect("discover json");
    assert!(advertised.contains("2025-06-18"), "{advertised}");
    assert!(!advertised.contains("2026-07-28"), "{advertised}");
    assert_eq!(replies[1]["result"]["protocolVersion"], "2025-06-18");
    let names: Vec<&str> = replies[2]["result"]["tools"]
        .as_array()
        .unwrap_or_else(|| panic!("tools/list is served: {}", replies[2]))
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    for expected in ["magistr_gate", "roadmap_next", "ship_check"] {
        assert!(names.contains(&expected), "{expected} in {names:?}");
    }
    assert!(
        replies[3].get("error").is_none(),
        "the legacy ping is served: {}",
        replies[3]
    );

    server.abort();
}

fn write_green_fixture(root: &Path) {
    write_fixture(root);
    let path = root.join(".magistr/project.json");
    let mut config: Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("project.json")).expect("json");
    config["quality_gate"] = json!({"check": "true", "test": "true", "lint": "true"});
    config["custom_gates"] = json!([{"name": "structure", "command": "true", "required": true}]);
    std::fs::write(&path, serde_json::to_string_pretty(&config).expect("json")).expect("write");
}

/// The acceptance of magistr-gate-forwards-ship-check, verbatim: with an
/// active task, a green `magistr_gate(all)` leaves one verified check per
/// gate on that task, and `magistr_set_phase(done)` is open right after it
/// with no manual `ship_check`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_green_gate_run_is_the_only_check_the_done_phase_needs() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("proj");
    std::fs::create_dir_all(&root).expect("root");
    write_green_fixture(&root);
    let data_dir = tmp.path().join("data");
    let (client, server) = pair(&root, &data_dir).await;

    call(
        &client,
        "ship_set_objective",
        json!({"description": "gate → ship_check"}),
    )
    .await;
    call(
        &client,
        "ship_plan",
        json!({"action": "add", "task_id": "gate-task", "title": "run gates"}),
    )
    .await;
    let started = call(&client, "ship_start", json!({"task_id": "gate-task"})).await;
    assert_ne!(started.is_error, Some(true), "{started:?}");

    // Before any gate ran, `done` is refused for want of a check.
    let refused = call(&client, "magistr_set_phase", json!({"phase": "done"})).await;
    assert_eq!(refused.is_error, Some(true), "{refused:?}");

    let recorded = call(&client, "magistr_gate", json!({"gate": "all"})).await;
    let body = text_json(&recorded);
    assert_eq!(body["passed"], true, "{body}");
    assert_eq!(body["recorded"], true, "every gate recorded: {body}");
    let gates = body["gates"].as_array().expect("gates");
    assert_eq!(
        gates.len(),
        4,
        "check, test, lint + the required custom gate"
    );
    for gate in gates {
        assert_eq!(gate["recorded"], true, "{gate}");
        assert_eq!(gate["verified"], true, "{gate}");
        assert_eq!(gate["exit_code"], 0, "{gate}");
    }

    // One verified, command-run check per gate, all on the active task.
    let ship = text_json(&call(&client, "ship_status", json!({})).await);
    let checks = ship["checks"].as_array().cloned().unwrap_or_default();
    assert_eq!(checks.len(), gates.len(), "{ship}");
    for gate in gates {
        let name = gate["gate"].as_str().expect("gate name");
        let matching: Vec<&Value> = checks.iter().filter(|c| c["name"] == name).collect();
        assert_eq!(matching.len(), 1, "one check for {name}: {checks:?}");
        assert_eq!(matching[0]["task_id"], "gate-task", "{checks:?}");
        assert_eq!(matching[0]["verified"], true, "{checks:?}");
        assert_eq!(matching[0]["passed"], true, "{checks:?}");
    }
    let task = ship["task_list"]
        .as_array()
        .and_then(|t| t.iter().find(|t| t["id"] == "gate-task"))
        .cloned()
        .expect("gate-task");
    assert_eq!(task["checks_count"], gates.len(), "{task}");

    // And `done` opens on those checks alone — no ship_check by hand.
    let done = call(&client, "magistr_set_phase", json!({"phase": "done"})).await;
    assert_ne!(done.is_error, Some(true), "{done:?}");

    let _ = client.cancel().await;
    server.abort();
}

/// When ship_check refuses — here, no active task — the gate still runs, and
/// the result says why it was not recorded in ship_check's own words rather
/// than a guessed cause, at the gate and at the top of an `all` run.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unrecorded_gate_names_ship_checks_refusal() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("proj");
    std::fs::create_dir_all(&root).expect("root");
    write_green_fixture(&root);
    let data_dir = tmp.path().join("data");
    let (client, server) = pair(&root, &data_dir).await;

    let one = text_json(&call(&client, "magistr_gate", json!({"gate": "check"})).await);
    assert_eq!(one["passed"], true, "{one}");
    assert_eq!(one["recorded"], false, "{one}");
    let note = one["note"].as_str().expect("note");
    assert!(note.starts_with("not recorded: "), "{note}");
    assert!(
        note.contains("no active task"),
        "ship_check's words: {note}"
    );

    let all = text_json(&call(&client, "magistr_gate", json!({"gate": "all"})).await);
    assert_eq!(all["passed"], true, "{all}");
    assert_eq!(all["recorded"], false, "{all}");
    let note = all["note"].as_str().expect("top-level note");
    assert!(note.contains("no active task"), "{note}");
    for gate in ["check", "test", "lint", "structure"] {
        assert!(note.contains(gate), "names every unrecorded gate: {note}");
    }

    let _ = client.cancel().await;
    server.abort();
}

/// A gate whose command counts its own runs, so "how many times did the gate
/// actually execute" is a number rather than an inference.
fn write_counting_fixture(root: &Path, counter: &Path) {
    write_fixture(root);
    let path = root.join(".magistr/project.json");
    let mut config: Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("project.json")).expect("json");
    // No quotes (see `write_fixture`); `/` so `sh` keeps the path whole.
    let tick = format!(
        "echo ran >> {}",
        counter.to_string_lossy().replace('\\', "/")
    );
    config["quality_gate"] = json!({"check": tick, "test": "true", "lint": "true"});
    config["custom_gates"] = json!([]);
    std::fs::write(&path, serde_json::to_string_pretty(&config).expect("json")).expect("write");
}

fn runs(counter: &Path) -> usize {
    std::fs::read_to_string(counter)
        .map(|s| s.lines().count())
        .unwrap_or(0)
}

/// A gate command runs **once** per `magistr_gate` call, whether or not there
/// is a ship task to record it on.
///
/// The regression: `ship_check` executes the command *before* it discovers it
/// has nowhere to record the result, so forwarding a gate to it with no active
/// task ran the command, threw the run away with a refusal, and left
/// `magistr_gate` to run it a second time. Two `cargo nextest` runs, two
/// clippy runs, two release builds per gate — and a timing or golden gate got
/// two independent chances to fail while only the second was reported.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_gate_runs_its_command_exactly_once_with_or_without_a_task() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("proj");
    std::fs::create_dir_all(&root).expect("root");
    let counter = tmp.path().join("runs.log");
    write_counting_fixture(&root, &counter);
    let data_dir = tmp.path().join("data");
    let (client, server) = pair(&root, &data_dir).await;

    // No objective, no plan, no active task: ship_check has nowhere to put
    // the result, and the gate must still run exactly once.
    let one = text_json(&call(&client, "magistr_gate", json!({"gate": "check"})).await);
    assert_eq!(one["passed"], true, "{one}");
    assert_eq!(one["recorded"], false, "{one}");
    assert_eq!(runs(&counter), 1, "unrecorded gate ran more than once");

    // And once more with a task, where the run happens inside ship_check.
    call(
        &client,
        "ship_set_objective",
        json!({"description": "count the runs"}),
    )
    .await;
    call(
        &client,
        "ship_plan",
        json!({"action": "add", "task_id": "t1", "title": "gate"}),
    )
    .await;
    call(&client, "ship_start", json!({"task_id": "t1"})).await;

    let two = text_json(&call(&client, "magistr_gate", json!({"gate": "check"})).await);
    assert_eq!(two["recorded"], true, "{two}");
    assert_eq!(runs(&counter), 2, "recorded gate ran more than once");

    let _ = client.cancel().await;
    server.abort();
}

/// The polyglot defect, end to end over the live gate path: a project whose
/// archetypes carry two stacks resolves one `test` gate into both stacks'
/// commands — the second stack's suite runs before anything is called
/// verified, and the workspace-wide command shared by several archetypes
/// runs once, not raced against itself.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_polyglot_gate_runs_every_stack_once() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("proj");
    std::fs::create_dir_all(&root).expect("root");
    write_fixture(&root);

    // Overlay archetypes: two rust archetypes naming the project's own test
    // command, plus a TS stack with its own.
    let path = root.join(".magistr/project.json");
    let mut config: Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("read")).expect("json");
    let archetype = |id: &str, arch_root: &str, test: &str| {
        json!({
            "id": id, "name": id, "language": "rust", "root": arch_root,
            "role": "library",
            "quality_gate": {"check": "true", "test": test, "lint": "true"},
            "format": {"extensions": ["rs"], "command": "true"},
            "testing": {"companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside"},
            "source_patterns": [], "test_patterns": []
        })
    };
    config["archetypes"] = json!([
        archetype("app", ".", "true"),
        archetype("core", "crates/core", "true"),
        archetype("app-typescript", ".", "echo bun-suite-ran"),
    ]);
    std::fs::write(&path, serde_json::to_string_pretty(&config).expect("json")).expect("write");

    let data_dir = tmp.path().join("data");
    let (client, server) = pair(&root, &data_dir).await;

    let result = call(&client, "magistr_gate", json!({"gate": "test"})).await;
    let body = text_json(&result);
    assert_eq!(body["passed"], true, "{body}");
    let gates = body["gates"].as_array().expect("grouped shape");
    let names: Vec<&str> = gates
        .iter()
        .map(|g| g["gate"].as_str().unwrap_or_default())
        .collect();
    assert_eq!(
        names,
        vec!["test", "test:app-typescript"],
        "one run for the shared command, one for the second stack"
    );
    let ts = &gates[1];
    assert!(
        ts["output"]
            .as_str()
            .unwrap_or_default()
            .contains("bun-suite-ran"),
        "the second stack's suite actually ran: {ts}"
    );

    client.cancel().await.expect("client cancel");
    server.abort();
}

/// A gate `magistr mcp-serve` runs directly is live on the feed socket the
/// cockpit attaches to: what the command writes goes out, raw, as
/// `ship`/`gate_output` frames keyed by the gate — on the pty the runner
/// spawns it on, so a line ends in the tty's `\r\n`.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_direct_gate_streams_its_bytes_on_the_feed_socket() {
    use std::time::{Duration, Instant};

    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::net::UnixStream;

    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("proj");
    std::fs::create_dir_all(&root).expect("root");
    write_fixture(&root);
    let data_dir = tmp.path().join("data");
    let sock = tmp.path().join("feed.sock");
    let tas = magistr_core::evidence::writer_config(&root, |_| None)
        .with_data_dir(&data_dir)
        .with_broadcast_path(Some(sock.clone()));
    let service = MagistrService::new(root.clone(), &tas)
        .await
        .expect("project owner");
    let (server_tx, client_tx) = tokio::io::duplex(1 << 16);
    let server = tokio::spawn(async move {
        let running = service.serve(server_tx).await.expect("server.serve");
        let _ = running.waiting().await;
    });
    let client = TestClient.serve(client_tx).await.expect("client.serve");

    let stream = UnixStream::connect(&sock)
        .await
        .expect("the feed socket is bound");
    let mut lines = BufReader::new(stream).lines();

    // Connecting is not being registered: until the library's accept loop
    // has run, a frame is dropped rather than queued. So the gate is run
    // until its bytes arrive, under one deadline.
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut bytes: Vec<u8> = Vec::new();
    while !bytes.windows(16).any(|w| w == b"mismatched types") {
        assert!(
            Instant::now() < deadline,
            "no gate_output frame carried the diagnostic; got {:?}",
            String::from_utf8_lossy(&bytes)
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
        let diag = text_json(&call(&client, "magistr_gate", json!({"gate": "diag"})).await);
        assert_eq!(diag["recorded"], false, "{diag}");
        assert_eq!(diag["exit_code"], 1, "{diag}");
        while let Ok(Ok(Some(line))) =
            tokio::time::timeout(Duration::from_millis(500), lines.next_line()).await
        {
            let frame: Value = serde_json::from_str(&line).expect("a json line");
            if frame["family"] == "magistr" && frame["type"] == "gate_output" {
                assert_eq!(frame["gate"], "diag", "{frame}");
                let chunk: Vec<u8> =
                    serde_json::from_value(frame["bytes"].clone()).expect("a byte array");
                bytes.extend(chunk);
            }
        }
    }
    assert!(
        bytes.windows(18).any(|w| w == b"mismatched types\r\n"),
        "the bytes are the pty's own, CRLF and all: {:?}",
        String::from_utf8_lossy(&bytes)
    );

    let _ = client.cancel().await;
    server.abort();
}

#[tokio::test]
async fn context_reads_owner_phase_and_canonical_evidence_without_mutation() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("project");
    std::fs::create_dir_all(&root).expect("root");
    write_fixture(&root);
    let (client, server) = pair(&root, &tmp.path().join("data")).await;
    let listed = client.peer().list_all_tools().await.expect("tools");
    assert!(listed.iter().any(|t| t.name == "magistr_context"));
    let before = text_json(&call(&client, "ship_status", json!({})).await);
    let empty = text_json(&call(&client, "magistr_context", json!({})).await);
    assert_eq!(empty["version"], 1);
    assert_eq!(
        empty["project_root"],
        root.canonicalize().unwrap().to_string_lossy().as_ref()
    );
    assert!(empty["phase"].is_null());
    assert!(
        empty["owner_epoch"]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .is_ok()
    );
    assert_eq!(
        before,
        text_json(&call(&client, "ship_status", json!({})).await)
    );
    assert!(
        empty["checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|g| g["key"] == "diag")
    );
    call(
        &client,
        "ship_set_objective",
        json!({"description": "Inspect evidence", "verification": "focused"}),
    )
    .await;
    call(
        &client,
        "ship_plan",
        json!({"action": "add", "task_id": "inspect", "title": "Inspect"}),
    )
    .await;
    call(&client, "ship_start", json!({"task_id": "inspect"})).await;
    call(&client, "magistr_set_phase", json!({"phase": "verify"})).await;
    let context = text_json(
        &call(
            &client,
            "magistr_context",
            json!({"task_id": "inspect", "receipt_id": "absent"}),
        )
        .await,
    );
    assert_eq!(context["phase"], "verifying");
    assert_eq!(context["phase_task"], "inspect");
    assert_eq!(context["task"]["record"]["id"], "inspect");
    assert_eq!(context["task"]["detail_limit"], 6000);
    assert!(context["receipt"].is_null());
    // Clearing the cycle invalidates its observation, even if a task id is reused.
    call(
        &client,
        "ship_set_objective",
        json!({"description": "Another cycle", "verification": "focused"}),
    )
    .await;
    let changed = text_json(&call(&client, "magistr_context", json!({})).await);
    assert!(changed["phase"].is_null());
    client.cancel().await.expect("cancel");
    server.await.expect("stopped");
}

#[tokio::test]
async fn context_refuses_malformed_and_unknown_task_requests_over_mcp() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("project");
    std::fs::create_dir_all(&root).expect("root");
    write_fixture(&root);
    let (client, server) = pair(&root, &tmp.path().join("data")).await;
    for args in [
        json!({"task_id": 1}),
        json!({"run": true}),
        json!({"task_id": "absent"}),
    ] {
        let mut request = CallToolRequestParams::new("magistr_context");
        request.arguments = args.as_object().cloned();
        assert!(client.peer().call_tool(request).await.is_err(), "{args}");
    }
    client.cancel().await.expect("cancel");
    server.await.expect("stopped");
}
