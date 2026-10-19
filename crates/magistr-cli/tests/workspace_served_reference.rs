//! The served surface against the pinned Bun reference server, side by side
//! in twin disposable workspaces: the discovered workspace input/output schemas, the status envelope, the empty
//! journal listing, the map plugin inventory and the run refusals. Ignored by
//! default — it needs the reference's Bun toolchain — and run explicitly by
//! the workspace-reference gate. Reference Bun tooling is development-only.
#[path = "support/unified.rs"]
mod unified;

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
    std::fs::create_dir_all(root.join(".magistr/workspace")).expect("state dir");
    std::fs::create_dir_all(root.join("src")).expect("src");
    std::fs::write(
        root.join(".magistr/workspace.toml"),
        "[commands]\nlist = \"echo hi\"\n",
    )
    .expect("allowlist");
    std::fs::write(root.join("src/app.ts"), "export const app = 1;\n").expect("source");
    std::fs::write(
        root.join(".magistr/workspace/plugins.json"),
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
        .env("MAGISTR_WORKSPACE_NETWORK_FS", "0");
    spawn(command).await
}

async fn native_client(root: &Path) -> (tokio::process::Child, Client) {
    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_magistr"));
    command
        .arg("mcp")
        .arg("--dir")
        .arg(root)
        .env("MAGISTR_WORKSPACE_NETWORK_FS", "0");
    spawn(command).await
}

async fn call(client: &Client, name: &str, args: Value) -> Value {
    let is_native = client.peer_info().is_some_and(|info| {
        info.server_info
            .as_ref()
            .is_some_and(|server| server.name == "magistr")
    });
    let (name, args, composed) = if is_native {
        unified::request(name, args)
    } else {
        (name.to_owned(), args, false)
    };
    let mut request = CallToolRequestParams::new(name.clone());
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
    unified::value(parsed, composed)
}

fn tool_view(tool: &Tool) -> Value {
    json!({
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

    // Workspace operation contracts are exposed through the unified catalog.
    let reference_tools: BTreeMap<String, Value> = reference
        .peer()
        .list_tools(None)
        .await
        .expect("reference tools")
        .tools
        .iter()
        .map(|tool| (tool.name.to_string(), tool_view(tool)))
        .collect();
    for (name, expected) in &reference_tools {
        let operation = call(
            &native,
            "discover",
            json!({"operation":name,"includeOutputSchema":true}),
        )
        .await;
        for field in ["inputSchema", "outputSchema"] {
            assert_eq!(
                &operation["operation"][field], &expected[field],
                "{name}: {field}"
            );
        }
    }

    // The status envelope, root aside, and the empty journal listing verbatim.
    assert_eq!(
        without_root(call(&native, "workspace_status", json!({})).await),
        without_root(call(&reference, "workspace_status", json!({})).await)
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
        json!({"command": "list", "args": [">", ".magistr/workspace.toml"]}),
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
