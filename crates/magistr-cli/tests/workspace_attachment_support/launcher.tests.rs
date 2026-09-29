//! Production launch and discovery reach the native workspace owner: the
//! retained `workbench-mcp` launch alias is this binary's bridge, the
//! cockpit's port reaches the live owner through `.mcp.json`, and the
//! legacy lock record migrates only through the explicit, refusing verb.
use super::*;

/// `magistr mcp-launch workbench-mcp <root>` with nothing else on PATH: no
/// Bun, no checkout, no external executable can be what answers.
fn launch(root: &std::path::Path, empty_path: &std::path::Path) -> tokio::process::Command {
    let mut command = magistr();
    command
        .args(["mcp-launch", "workbench-mcp"])
        .arg(root)
        .env("PATH", empty_path)
        .env_remove("WORKBENCH_MCP")
        .env_remove("WORKBENCH_MCP_REPO");
    command
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_workbench_launch_alias_serves_the_native_bridge_and_a_second_launch_attaches() {
    let dir = workspace();
    let empty = tempfile::tempdir().expect("an empty PATH directory");
    let owner = Connection::open_with(launch(dir.path(), empty.path())).await;
    owner.assert_tools().await;
    assert_eq!(generation(dir.path()), 1);
    let attached = Connection::open_with(launch(dir.path(), empty.path())).await;
    attached.assert_tools().await;
    assert_eq!(generation(dir.path()), 1, "the second launch attached");
    assert_ok(&attached.call("read", json!({"file": "alice.txt"})).await);
    assert_ok(&owner.call("workbench_status", json!({})).await);
    attached.close().await;
    owner.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_cockpit_port_reaches_the_live_owner_through_the_projects_mcp_json() {
    let dir = workspace();
    std::fs::write(
        dir.path().join(".mcp.json"),
        json!({"mcpServers": {"workbench-mcp": {
            "command": binary(),
            "args": ["mcp-launch", "workbench-mcp", "."],
            "type": "stdio",
        }}})
        .to_string(),
    )
    .expect("project launch line");
    let owner = Connection::open(dir.path()).await;
    assert_ok(&owner.call("edit", edit("alice.txt", "agent bytes")).await);
    let root = dir.path().to_path_buf();
    let (map, read, journal) = tokio::task::spawn_blocking(move || {
        let mut port = magistr::orchestration::commands::workbench_port::workbench_port(&root)
            .expect("the .mcp.json entry becomes a port");
        let map = port.map().expect("map through the live owner");
        let read = port.read("alice.txt").expect("read through the live owner");
        let journal = port.journal().expect("the native journal verb answers");
        (map, read, journal)
    })
    .await
    .expect("cockpit port task");
    assert!(map.iter().any(|entry| entry == "alice.txt"), "{map:?}");
    // The port joins the read's line segments; the file's own bytes end in
    // a newline the port does not add back.
    assert_eq!(read, "agent bytes");
    assert_eq!(journal.len(), 1, "{journal:?}");
    assert_eq!(journal[0].path, "alice.txt");
    assert_eq!(
        generation(dir.path()),
        1,
        "the cockpit attached, never admitted"
    );
    owner.close().await;
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn workspace_migrate_refuses_a_live_legacy_record_and_moves_a_dead_one_aside() {
    let dir = workspace();
    let state = dir.path().join(".workbench");
    std::fs::create_dir(&state).expect("state");
    let lock = state.join("server.lock");
    let live = format!(
        r#"{{"pid":{},"host":"{}"}}"#,
        std::process::id(),
        hostname()
    );
    std::fs::write(&lock, &live).expect("live legacy record");
    let refused = migrate(dir.path()).await;
    assert_eq!(refused.status.code(), Some(75));
    assert!(String::from_utf8_lossy(&refused.stderr).contains("alive"));
    assert_eq!(std::fs::read_to_string(&lock).expect("preserved"), live);

    let record = dead_legacy_record();
    std::fs::write(&lock, &record).expect("dead legacy record");
    let migrated = migrate(dir.path()).await;
    assert_eq!(
        migrated.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&migrated.stderr)
    );
    assert!(!lock.exists());
    let backups: Vec<_> = std::fs::read_dir(&state)
        .expect("state listing")
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("server.lock.legacy-")
        })
        .collect();
    assert_eq!(backups.len(), 1);
    assert_eq!(
        std::fs::read_to_string(backups[0].path()).expect("backup"),
        record
    );
    let owner = Connection::open(dir.path()).await;
    assert_eq!(generation(dir.path()), 1);
    owner.close().await;
}
