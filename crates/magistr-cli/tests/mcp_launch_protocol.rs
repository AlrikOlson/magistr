//! Exercise the generated launch argv with real MCP clients and disposable stores.

#[path = "support/magistr_command.rs"]
mod magistr_command;

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use magistr::detect::detect_stack;
use magistr::scaffold::WorkbenchCommand;
use magistr::scaffold::pipeline::ScaffoldPipeline;
use rmcp::model::{CallToolRequestParams, CallToolResult};
use rmcp::{ClientHandler, ServiceExt};
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::process::{Child, Command};
use tokio::time::timeout;

const DEADLINE: Duration = Duration::from_secs(30);
const SENTINEL: &str = "portable_protocol_workspace_marker";

#[derive(Clone)]
struct TestClient;
impl ClientHandler for TestClient {}

type Client = rmcp::service::RunningService<rmcp::RoleClient, TestClient>;

struct Fixture {
    temporary: TempDir,
    root: PathBuf,
    config: Value,
}

impl Fixture {
    fn relocated() -> Self {
        let temporary = tempfile::tempdir().expect("isolated protocol fixture");
        let original = temporary.path().join("original project");
        std::fs::create_dir_all(original.join("src/nested")).expect("source directories");
        std::fs::write(
            original.join("Cargo.toml"),
            "[package]\nname = \"mcp-protocol-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .expect("manifest");
        std::fs::write(
            original.join("src/lib.rs"),
            format!("pub fn {SENTINEL}() {{}}\n"),
        )
        .expect("root-only source marker");
        let stack = detect_stack(&original).expect("detect fixture");
        ScaffoldPipeline::headless(stack)
            .generate(&original)
            .expect("generate the real host configuration");
        let config_bytes = std::fs::read(original.join(".mcp.json")).expect("generated MCP config");
        let config = serde_json::from_slice(&config_bytes).expect("valid MCP config");
        let root = temporary.path().join("relocated project λ 東京");
        std::fs::rename(&original, &root).expect("relocate without regenerating");
        assert_eq!(
            std::fs::read(root.join(".mcp.json")).expect("relocated MCP config"),
            config_bytes
        );
        Self {
            temporary,
            root,
            config,
        }
    }

    fn command(&self, target: &str, cwd: &Path) -> Command {
        let entry = &self.config["mcpServers"][target];
        assert_eq!(entry["command"], "magistr");
        let args: Vec<&str> = entry["args"]
            .as_array()
            .expect("generated argv")
            .iter()
            .map(|arg| arg.as_str().expect("string argument"))
            .collect();
        assert_eq!(args, ["mcp-launch", target, "."]);
        let home = self.temporary.path().join("home");
        std::fs::create_dir_all(&home).expect("isolated home");
        // Select the binary under test in place of the installed `magistr`;
        // every argument still comes directly from the generated host entry.
        let mut command = Command::from(magistr_command::magistr_command());
        command
            .args(args)
            .current_dir(cwd)
            .env("HOME", &home)
            .env("USERPROFILE", &home)
            .env("XDG_CONFIG_HOME", home.join("config"))
            .env("XDG_DATA_HOME", home.join("data"))
            .env("THINK_AND_SHIP_DATA_DIR", home.join("think-and-ship"))
            .env("THINK_AND_SHIP_PERSIST", "true")
            .env(
                "THINK_AND_SHIP_BROADCAST_PATH",
                self.temporary.path().join("feed.sock"),
            )
            .env_remove("MAGISTR_LOG")
            .env_remove("MAGISTR_LOG_FILE")
            .env("NO_COLOR", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true);
        command
    }

    fn assert_no_roadmap_files(&self) {
        for base in [&self.root, &self.root.join("src/nested")] {
            for path in ["ROADMAP.md", "ROADMAP.json", ".magistr/roadmap.yaml"] {
                assert!(!base.join(path).exists(), "unexpected roadmap file: {path}");
            }
        }
    }
}

async fn connect(mut command: Command) -> (Client, Child) {
    let mut child = command.spawn().expect("start generated MCP command");
    let stdout = child.stdout.take().expect("piped protocol output");
    let stdin = child.stdin.take().expect("piped protocol input");
    let client = timeout(DEADLINE, TestClient.serve((stdout, stdin)))
        .await
        .expect("MCP initialization deadline")
        .expect("MCP initialization");
    (client, child)
}

async fn call(client: &Client, name: &str, arguments: Value) -> Value {
    let mut request = CallToolRequestParams::new(name.to_owned());
    request.arguments = arguments.as_object().cloned();
    let result = timeout(DEADLINE, client.peer().call_tool(request))
        .await
        .expect("tool call deadline")
        .expect("tool response");
    assert_ne!(result.is_error, Some(true), "{name}: {result:?}");
    let body = text_json(&result);
    assert_ne!(body["ok"], false, "{name}: {body}");
    body
}

fn text_json(result: &CallToolResult) -> Value {
    if let Some(value) = &result.structured_content {
        return value.clone();
    }
    let text = result
        .content
        .iter()
        .find_map(|content| content.as_text().map(|text| text.text.as_str()))
        .expect("JSON text response");
    serde_json::from_str(text).expect("valid response JSON")
}

async fn stop(client: Client, mut child: Child) {
    timeout(DEADLINE, client.cancel())
        .await
        .expect("client cancellation deadline")
        .expect("cancel client");
    match timeout(Duration::from_secs(5), child.wait()).await {
        Ok(status) => assert!(status.expect("reap MCP server").success()),
        Err(_) => {
            child
                .kill()
                .await
                .expect("kill server after transport closed");
            child.wait().await.expect("reap killed server");
        }
    }
}

#[tokio::test]
async fn generated_magistr_launch_serves_native_tools_after_relocation_from_root_and_nested_cwd() {
    let fixture = Fixture::relocated();
    let identity: Value = serde_json::from_slice(
        &std::fs::read(fixture.root.join(".think-and-ship/project.json"))
            .expect("project identity"),
    )
    .expect("identity JSON");
    for cwd in [&fixture.root, &fixture.root.join("src/nested")] {
        let (client, child) = connect(fixture.command("magistr", cwd)).await;
        let tools = timeout(DEADLINE, client.peer().list_all_tools())
            .await
            .expect("tools/list deadline")
            .expect("list all native tools");
        for name in [
            "think_record_step",
            "ship_status",
            "roadmap_status",
            "magistr_gate",
        ] {
            assert!(tools.iter().any(|tool| tool.name == name), "missing {name}");
        }
        let status = call(&client, "roadmap_status", json!({})).await;
        assert_eq!(status["project_id"], identity["id"], "workspace identity");
        stop(client, child).await;
        fixture.assert_no_roadmap_files();
    }
}

#[tokio::test]
#[ignore = "requires installed Workbench"]
async fn generated_workbench_launch_reads_and_searches_relocated_workspace_from_root_and_nested_cwd()
 {
    let workbench = WorkbenchCommand::resolve();
    assert!(
        workbench.available,
        "install Workbench before running this probe"
    );
    // Each launch gets its own workspace so an installed server's shutdown
    // cannot leave the following probe waiting on its previous workspace lock.
    for nested in [false, true] {
        let fixture = Fixture::relocated();
        let cwd = if nested {
            fixture.root.join("src/nested")
        } else {
            fixture.root.clone()
        };
        let mut command = fixture.command("workbench-mcp", &cwd);
        if workbench.leading_args.is_empty() {
            command.env("WORKBENCH_MCP", &workbench.command);
        } else {
            // Preserve a home-checkout resolution across the isolated HOME.
            let main = Path::new(&workbench.leading_args[1]);
            let repo = main
                .parent()
                .and_then(Path::parent)
                .expect("Workbench checkout");
            command
                .env_remove("WORKBENCH_MCP")
                .env("WORKBENCH_MCP_REPO", repo);
        }
        let (client, child) = connect(command).await;
        let tools = timeout(DEADLINE, client.peer().list_all_tools())
            .await
            .expect("tools/list deadline")
            .expect("list Workbench tools");
        for name in ["read", "search", "workbench_status"] {
            assert!(tools.iter().any(|tool| tool.name == name), "missing {name}");
        }
        let read = call(&client, "read", json!({"file": "src/lib.rs"})).await;
        assert!(
            read.to_string().contains(SENTINEL),
            "root source read: {read}"
        );
        let search = call(
            &client,
            "search",
            json!({"query": SENTINEL, "mode": "lexical", "path": "src/**"}),
        )
        .await;
        assert!(
            search.to_string().contains("src/lib.rs"),
            "root search: {search}"
        );
        assert!(
            search.to_string().contains(SENTINEL),
            "source match: {search}"
        );
        stop(client, child).await;
        fixture.assert_no_roadmap_files();
    }
}
