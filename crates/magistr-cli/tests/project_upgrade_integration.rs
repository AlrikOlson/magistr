//! Saved registrations and repair exercise automatic upgrades through the real CLI.
#![cfg(unix)]
#[path = "support/magistr_command.rs"]
mod magistr_command;
use magistr_workspace::{
    journal::{JournalDb, JournalDbOptions, JournalOpInit, JournalReader},
    service::ServerLock,
};
use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir(root.join(".git")).unwrap();
    magistr::scaffold::pipeline::ScaffoldPipeline::headless(magistr::types::Stack::default())
        .with_agents(&["claude".into()])
        .generate(root)
        .unwrap();
    let path = root.join(".magistr/project.json");
    let mut config: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    config["roadmap_backend"] = "think-and-ship".into();
    config["quality_gate"]["test"] = "my-project-test".into();
    std::fs::write(path, serde_json::to_string_pretty(&config).unwrap()).unwrap();
    std::fs::write(root.join(".mcp.json"), r#"{"mcpServers":{"magistr":{"command":"magistr","args":["mcp-launch","magistr","."],"type":"stdio"},"custom":{"command":"custom"}}}"#).unwrap();
    std::fs::create_dir(root.join(".workbench")).unwrap();
    let lock = ServerLock::acquire(root.join(".workbench/server.lock")).unwrap();
    let mut journal =
        JournalDb::open(JournalDbOptions::new(root.join(".workbench/journal.db"))).unwrap();
    journal
        .append(&JournalOpInit {
            tool: "edit".into(),
            args_hash: "saved".into(),
            session_id: "old-session".into(),
            undo_of: None,
            files: vec![],
        })
        .unwrap();
    drop(journal);
    drop(lock);
    dir
}

fn command(root: &Path) -> Command {
    let mut cmd = magistr_command::magistr_command();
    cmd.current_dir(root)
        .env("MAGISTR_WORKFLOW_DATA_DIR", root.join("state"))
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("XDG_DATA_HOME", root.join("data"))
        .env("MAGISTR_WORKSPACE_NETWORK_FS", "0")
        .env("NO_COLOR", "1");
    cmd
}

fn initialized(root: &Path, args: &[&str]) {
    let mut child = command(root)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-06-18\",\"capabilities\":{},\"clientInfo\":{\"name\":\"upgrade-test\",\"version\":\"0\"}}}\n{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n").unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    for line in stdout.lines() {
        let value: serde_json::Value =
            serde_json::from_str(line).expect("stdout contains only protocol messages");
        assert_eq!(value["jsonrpc"], "2.0");
    }
    assert!(stdout.contains("\"id\":1"), "{stdout}");
    assert!(
        !String::from_utf8_lossy(&out.stderr).contains("upgrad"),
        "routine upgrades are quiet"
    );
}

#[test]
fn saved_launch_command_upgrades_configuration_and_history_before_serving() {
    let dir = fixture();
    let root = dir.path();
    let before = std::fs::read(root.join(".magistr/project.json")).unwrap();
    let preview = command(root)
        .args(["repair", ".", "--dry-run"])
        .output()
        .unwrap();
    assert!(
        preview.status.success(),
        "{}",
        String::from_utf8_lossy(&preview.stderr)
    );
    assert_eq!(
        std::fs::read(root.join(".magistr/project.json")).unwrap(),
        before
    );
    assert!(!root.join(".magistr/upgrades").exists());
    assert!(!root.join(".magistr/workspace/journal.db").exists());
    initialized(root, &["mcp-launch", "magistr", "."]);
    let config: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join(".magistr/project.json")).unwrap())
            .unwrap();
    assert_eq!(config["roadmap_backend"], "native");
    assert_eq!(config["quality_gate"]["test"], "my-project-test");
    let mcp: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join(".mcp.json")).unwrap()).unwrap();
    assert_eq!(
        mcp["mcpServers"]["magistr"]["args"],
        serde_json::json!(["mcp", "--dir", "."])
    );
    assert_eq!(mcp["mcpServers"]["custom"]["command"], "custom");
    let history = JournalReader::open(root.join(".magistr/workspace/journal.db"))
        .unwrap()
        .list(0)
        .unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].session_id, "old-session");
    initialized(root, &["mcp", "--dir", "."]);
    assert_eq!(
        JournalReader::open(root.join(".magistr/workspace/journal.db"))
            .unwrap()
            .list(0)
            .unwrap(),
        history
    );
}

#[test]
fn explicit_repair_applies_the_same_upgrade_and_checks_the_new_connection() {
    let dir = fixture();
    let root = dir.path();
    let out = command(root)
        .args(["repair", ".", "--yes"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("Updated"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        JournalReader::open(root.join(".magistr/workspace/journal.db"))
            .unwrap()
            .list(0)
            .unwrap()
            .len(),
        1
    );
}
