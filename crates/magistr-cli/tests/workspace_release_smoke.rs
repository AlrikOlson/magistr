//! The shipping artifact, smoked (chunk arch-14c10b): the release-profile
//! `magistr` binary serving a disposable workspace over its own stdio with
//! nothing beside it — a PATH holding only `echo`, so no Bun, Node, N-API
//! addon or addon-to-WASM fallback can be reached — driven through
//! initialize, tools/list and every core tool once.
//!
//! Ignored by default: `scripts/workspace-release-smoke.sh` runs it under
//! `--release`, and the test refuses a debug-profile binary rather than
//! passing as a proxy for the shipping one.
#[path = "support/magistr_command.rs"]
mod magistr_command;

use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use magistr_command::magistr_command;
use rmcp::{
    ServiceExt,
    model::CallToolRequestParams,
    service::{RoleClient, RunningService},
};
use serde_json::{Value, json};

const IGNORE: &str =
    "release-profile smoke of the shipping binary: ./scripts/workspace-release-smoke.sh";
const CORE_TOOLS: [&str; 10] = [
    "workbench_status",
    "map",
    "read",
    "search",
    "edit",
    "files",
    "write",
    "undo",
    "script",
    "run",
];
const APP_TS: &str = "export function app(): number {\n  return 1;\n}\n";

/// The only executable the served binary may reach: `echo`, for the run
/// tool. Bun, Node and every other host runtime are unreachable by
/// construction, which is what "Bun-free" means for a process.
fn scrubbed_path() -> tempfile::TempDir {
    let bin = tempfile::tempdir().expect("scrubbed PATH");
    let echo = which_system("echo");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&echo, bin.path().join("echo")).expect("echo on the scrubbed PATH");
    #[cfg(not(unix))]
    std::fs::copy(&echo, bin.path().join("echo.exe")).expect("echo on the scrubbed PATH");
    for runtime in ["bun", "node", "bunx", "npx"] {
        assert!(
            !bin.path().join(runtime).exists(),
            "{runtime} must not be reachable"
        );
    }
    bin
}

fn which_system(name: &str) -> PathBuf {
    ["/bin", "/usr/bin"]
        .iter()
        .map(|dir| Path::new(dir).join(name))
        .find(|candidate| candidate.is_file())
        .unwrap_or_else(|| panic!("{name} is not under /bin or /usr/bin"))
}

fn workspace() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("workspace");
    std::fs::create_dir_all(dir.path().join(".git")).expect("repository marker");
    std::fs::create_dir_all(dir.path().join("src")).expect("src");
    std::fs::write(dir.path().join("src/app.ts"), APP_TS).expect("source");
    std::fs::write(
        dir.path().join("workbench.toml"),
        "[commands]\nhello = \"echo hello-from-the-release-binary\"\n",
    )
    .expect("allowlist");
    dir
}

/// The release binary as a tokio child over its own stdio, seeing only the
/// scrubbed PATH.
async fn served(
    root: &Path,
    path: &Path,
) -> (tokio::process::Child, RunningService<RoleClient, ()>) {
    let mut command = magistr_command();
    command
        .arg("workspace")
        .arg("serve")
        .arg(root)
        .env("WORKBENCH_NETWORK_FS", "0")
        .env("PATH", path);
    let mut child = tokio::process::Command::from(command)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn the release binary");
    let stdin = child.stdin.take().expect("child stdin");
    let stdout = child.stdout.take().expect("child stdout");
    let client = ().serve((stdout, stdin)).await.expect("initialize");
    (child, client)
}

fn release_binary() -> &'static str {
    let binary = env!("CARGO_BIN_EXE_magistr");
    assert!(
        !cfg!(debug_assertions)
            && Path::new(binary)
                .components()
                .any(|part| part.as_os_str() == "release"),
        "the smoke must run the release binary, not a debug proxy: {binary}"
    );
    binary
}

async fn call(client: &RunningService<RoleClient, ()>, name: &str, args: Value) -> Value {
    let mut request = CallToolRequestParams::new(name.to_owned());
    request.arguments = args.as_object().cloned();
    let result = client.peer().call_tool(request).await.expect("tool call");
    result.structured_content.expect("structured envelope")
}

#[ignore = "release-profile smoke of the shipping binary: ./scripts/workspace-release-smoke.sh"]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_release_binary_serves_every_core_tool_with_no_runtime_beside_it() {
    let _ = IGNORE;
    let binary = release_binary();
    let path = scrubbed_path();
    let dir = workspace();
    let (mut child, client) = served(dir.path(), path.path()).await;
    let info = client.peer().peer_info().expect("server info");
    assert_eq!(
        info.server_info.as_ref().map(|server| server.name.as_str()),
        Some("magistr"),
        "{binary}"
    );
    let tools = client
        .peer()
        .list_tools(None)
        .await
        .expect("tools/list")
        .tools;
    let names: Vec<&str> = tools.iter().map(|tool| tool.name.as_ref()).collect();
    assert_eq!(names, CORE_TOOLS, "the retained surface, in order");

    let status = call(&client, "workbench_status", json!({})).await;
    assert_eq!(status["ok"], true, "{status}");
    assert_eq!(status["results"][0]["journalMode"], "WAL");

    let map = call(&client, "map", json!({"depth": 2})).await;
    assert_eq!(map["ok"], true, "{map}");
    assert_eq!(map["results"][0]["kind"], "overview");

    let read = call(&client, "read", json!({"file": "src/app.ts"})).await;
    assert_eq!(read["ok"], true, "{read}");
    assert_eq!(read["results"][0]["file"], "src/app.ts");

    let search = call(
        &client,
        "search",
        json!({"query": "return", "mode": "lexical"}),
    )
    .await;
    assert_eq!(search["ok"], true, "{search}");
    assert_eq!(search["results"][0]["file"], "src/app.ts");

    let edited = call(
        &client,
        "edit",
        json!({"op": "replace_text", "file": "src/app.ts", "old": "return 1;", "new": "return 3;"}),
    )
    .await;
    assert_eq!(edited["ok"], true, "{edited}");
    assert!(
        std::fs::read_to_string(dir.path().join("src/app.ts"))
            .expect("edited")
            .contains("return 3;")
    );

    let created = call(
        &client,
        "files",
        json!({"op": "create", "to": "src/fresh.ts", "autoApply": true}),
    )
    .await;
    assert_eq!(created["ok"], true, "{created}");
    assert!(dir.path().join("src/fresh.ts").is_file());

    let listed = call(
        &client,
        "undo",
        json!({"action": "list", "scope": "workspace"}),
    )
    .await;
    assert_eq!(listed["ok"], true, "{listed}");
    let newest = listed["results"][0]["seq"].as_i64().expect("newest seq");
    assert_eq!(listed["results"][0]["tool"], "files");
    let rewound = call(&client, "undo", json!({"ops": [newest]})).await;
    assert_eq!(rewound["ok"], true, "{rewound}");
    assert!(
        !dir.path().join("src/fresh.ts").exists(),
        "the creation was rewound"
    );

    let script = call(
        &client,
        "script",
        json!({"source": "return workspace.glob(\"**/*.ts\");"}),
    )
    .await;
    assert_eq!(script["ok"], true, "{script}");
    assert_eq!(script["results"][0]["value"], json!(["src/app.ts"]));

    let ran = call(&client, "run", json!({"command": "hello"})).await;
    assert_eq!(ran["ok"], true, "{ran}");
    assert_eq!(ran["results"][0]["exitCode"], 0, "{ran}");
    assert!(
        ran.to_string().contains("hello-from-the-release-binary"),
        "{ran}"
    );

    client.cancel().await.expect("client closes");
    let status = tokio::time::timeout(Duration::from_secs(30), child.wait())
        .await
        .expect("the server exits when its client leaves")
        .expect("wait");
    assert!(status.success(), "{status}");
}
