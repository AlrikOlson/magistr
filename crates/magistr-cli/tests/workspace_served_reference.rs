//! The served surface against the pinned Bun reference server, side by side
//! in twin disposable workspaces: the tool list (names, titles,
//! descriptions, input and output schemas), the status envelope, the empty
//! journal listing, the map plugin inventory and the run refusals. Ignored by
//! default — it needs the reference's Bun toolchain — and run explicitly by
//! the workspace-reference gate. Reference Bun tooling is development-only.
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Stdio,
};

use rmcp::{
    ServiceExt,
    model::{CallToolRequestParams, Tool},
    service::{RoleClient, RunningService},
};
use serde_json::{Value, json};

type Client = RunningService<RoleClient, ()>;

/// A server as a tokio child whose stdin and stdout are the client's transport.
async fn spawn(mut command: tokio::process::Command) -> (tokio::process::Child, Client) {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn a server");
    let stdin = child.stdin.take().expect("child stdin");
    let stdout = child.stdout.take().expect("child stdout");
    let client = ().serve((stdout, stdin)).await.expect("initialize");
    (child, client)
}

fn reference_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../subsystems/workspace/reference")
}

fn seed(root: &Path) {
    std::fs::create_dir_all(root.join(".git")).expect("repository marker");
    std::fs::create_dir_all(root.join(".workbench")).expect("state dir");
    std::fs::create_dir_all(root.join("src")).expect("src");
    std::fs::write(
        root.join("workbench.toml"),
        "[commands]\nlist = \"echo hi\"\n",
    )
    .expect("allowlist");
    std::fs::write(root.join("src/app.ts"), "export const app = 1;\n").expect("source");
    std::fs::write(
        root.join(".workbench/plugins.json"),
        json!({"plugins": [
            {"name": "local.hack", "source": "sideload", "tier": "blessed"},
            {"name": "acme.signed", "version": "2.0.0", "source": "registry", "tier": "verified"},
            {"name": "acme.plain", "source": "registry"}
        ]})
        .to_string(),
    )
    .expect("inventory");
}

async fn reference_client(root: &Path) -> (tokio::process::Child, Client) {
    let mut command = tokio::process::Command::new("bun");
    command
        .arg(reference_dir().join("src/main.ts"))
        .arg(root)
        .current_dir(reference_dir())
        .env("WORKBENCH_NETWORK_FS", "0");
    spawn(command).await
}

async fn native_client(root: &Path) -> (tokio::process::Child, Client) {
    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_magistr"));
    command
        .arg("workspace")
        .arg("serve")
        .arg(root)
        .env("WORKBENCH_NETWORK_FS", "0");
    spawn(command).await
}

async fn call(client: &Client, name: &str, args: Value) -> Value {
    let mut request = CallToolRequestParams::new(name.to_owned());
    request.arguments = args.as_object().cloned();
    let result = client.peer().call_tool(request).await.expect("tool call");
    let text = result
        .content
        .iter()
        .find_map(|block| block.as_text().map(|text| text.text.clone()))
        .expect("text");
    let parsed: Value = serde_json::from_str(&text).expect("json envelope");
    assert_eq!(
        Some(&parsed),
        result.structured_content.as_ref(),
        "{name}: text and structuredContent agree"
    );
    parsed
}

fn tool_view(tool: &Tool) -> Value {
    json!({
        "title": tool.title,
        "description": tool.description,
        "inputSchema": Value::Object((*tool.input_schema).clone()),
        "outputSchema": tool.output_schema.as_ref().map(|schema| Value::Object((**schema).clone())),
    })
}

/// Roots differ between the twin workspaces; everything else must not.
fn without_root(mut envelope: Value) -> Value {
    if let Some(row) = envelope["results"].get_mut(0) {
        row.as_object_mut().map(|row| row.remove("root"));
    }
    envelope
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires the reference's Bun toolchain; the workspace-reference gate runs this explicitly"]
async fn the_native_server_matches_the_reference_over_the_wire() {
    let twins = tempfile::tempdir().expect("twins");
    let reference_root = twins.path().join("reference");
    let native_root = twins.path().join("native");
    seed(&reference_root);
    seed(&native_root);
    let (mut reference_child, reference) = reference_client(&reference_root).await;
    let (mut native_child, native) = native_client(&native_root).await;

    // The declared surface: same names, titles, descriptions and schemas.
    let reference_tools: BTreeMap<String, Value> = reference
        .peer()
        .list_tools(None)
        .await
        .expect("reference tools")
        .tools
        .iter()
        .map(|tool| (tool.name.to_string(), tool_view(tool)))
        .collect();
    let native_tools: BTreeMap<String, Value> = native
        .peer()
        .list_tools(None)
        .await
        .expect("native tools")
        .tools
        .iter()
        .map(|tool| (tool.name.to_string(), tool_view(tool)))
        .collect();
    assert_eq!(
        native_tools.keys().collect::<Vec<_>>(),
        reference_tools.keys().collect::<Vec<_>>()
    );
    for (name, expected) in &reference_tools {
        assert_eq!(
            &native_tools[name], expected,
            "{name}: served declaration differs"
        );
    }

    // The status envelope, root aside, and the empty journal listing verbatim.
    assert_eq!(
        without_root(call(&native, "workbench_status", json!({})).await),
        without_root(call(&reference, "workbench_status", json!({})).await)
    );
    assert_eq!(
        call(&native, "undo", json!({"action": "list"})).await,
        call(&reference, "undo", json!({"action": "list"})).await
    );

    // The installed-plugin inventory with its trust labels and the note.
    let native_map = call(&native, "map", json!({"depth": 1, "detail": "skeleton"})).await;
    let reference_map = call(&reference, "map", json!({"depth": 1, "detail": "skeleton"})).await;
    assert_eq!(
        native_map["results"][0]["plugins"],
        reference_map["results"][0]["plugins"]
    );
    let untrusted = |envelope: &Value| {
        envelope["notes"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .find(|note| note.contains("sideloaded"))
            .map(str::to_owned)
    };
    assert_eq!(untrusted(&native_map), untrusted(&reference_map));
    assert!(untrusted(&native_map).is_some());

    // Typed run refusals agree on code and recovery; messages are each side's own.
    for args in [
        json!({"command": "deploy"}),
        json!({"command": "list", "args": [">", "workbench.toml"]}),
    ] {
        let native_run = call(&native, "run", args.clone()).await;
        let reference_run = call(&reference, "run", args).await;
        assert_eq!(native_run["ok"], false);
        assert_eq!(native_run["error"]["code"], reference_run["error"]["code"]);
        assert_eq!(
            native_run["error"]["recovery"],
            reference_run["error"]["recovery"]
        );
    }
    let _ = native.cancel().await;
    let _ = reference.cancel().await;
    let _ = native_child.wait().await;
    let _ = reference_child.wait().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires the reference's Bun toolchain; the workspace-reference gate runs this explicitly"]
async fn native_and_legacy_owners_refuse_each_other_without_touching_the_live_lock() {
    use std::time::Duration;
    let dir = tempfile::tempdir().expect("workspace");
    seed(dir.path());
    let (mut native_child, native) = native_client(dir.path()).await;
    let lock = dir.path().join(".workbench/server.lock");
    let native_marker = std::fs::read(&lock).expect("native marker");
    let legacy = tokio::time::timeout(
        Duration::from_secs(15),
        tokio::process::Command::new("bun")
            .arg(reference_dir().join("src/main.ts"))
            .arg(dir.path())
            .current_dir(reference_dir())
            .stdin(Stdio::null())
            .output(),
    )
    .await
    .expect("bounded legacy refusal")
    .expect("legacy contender");
    assert_eq!(legacy.status.code(), Some(75));
    assert_eq!(std::fs::read(&lock).expect("marker"), native_marker);
    assert_eq!(
        call(&native, "workbench_status", json!({})).await["ok"],
        true
    );
    native.cancel().await.expect("native detach");
    assert!(
        tokio::time::timeout(Duration::from_secs(30), native_child.wait())
            .await
            .expect("native exits")
            .expect("native wait")
            .success()
    );

    let legacy_dir = tempfile::tempdir().expect("legacy workspace");
    seed(legacy_dir.path());
    let (mut legacy_child, legacy_client) = reference_client(legacy_dir.path()).await;
    let lock = legacy_dir.path().join(".workbench/server.lock");
    let legacy_marker = std::fs::read(&lock).expect("legacy marker");
    // The native bridge is refused admission, serves discovery instead of
    // exiting 75 (audit-260912-20), writes nothing on its stdio for a client
    // that sends nothing, and ends cleanly on that client's EOF — never
    // touching the live legacy lock.
    let native = tokio::time::timeout(
        Duration::from_secs(15),
        tokio::process::Command::new(env!("CARGO_BIN_EXE_magistr"))
            .args(["workspace", "serve"])
            .arg(legacy_dir.path())
            .stdin(Stdio::null())
            .output(),
    )
    .await
    .expect("bounded native refusal")
    .expect("native contender");
    assert_eq!(native.status.code(), Some(0), "{native:?}");
    assert!(native.stdout.is_empty(), "no MCP frame without a request");
    let diagnostic = String::from_utf8_lossy(&native.stderr);
    assert!(diagnostic.contains("migrat"), "{diagnostic}");
    assert!(diagnostic.contains("serving discovery"), "{diagnostic}");
    assert_eq!(std::fs::read(&lock).expect("marker"), legacy_marker);
    assert_eq!(
        call(&legacy_client, "workbench_status", json!({})).await["ok"],
        true
    );
    legacy_client.cancel().await.expect("legacy detach");
    assert!(
        tokio::time::timeout(Duration::from_secs(30), legacy_child.wait())
            .await
            .expect("legacy exits")
            .expect("legacy wait")
            .success()
    );
}
