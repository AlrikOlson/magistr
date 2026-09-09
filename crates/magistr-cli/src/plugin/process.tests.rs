use std::os::unix::fs::PermissionsExt;

use tempfile::TempDir;

use super::*;
use crate::plugin::types::{PluginRequest, PluginResponse};

/// Create a mock plugin script that echoes back init_ok for any init request.
fn create_echo_plugin(dir: &std::path::Path) -> String {
    let script = r#"#!/bin/sh
# Simple echo plugin: reads JSON lines from stdin, responds with init_ok
while IFS= read -r line; do
    id=$(echo "$line" | sed -n 's/.*"id":\([0-9]*\).*/\1/p')
    type=$(echo "$line" | sed -n 's/.*"type":"\([^"]*\)".*/\1/p')
    case "$type" in
        init)
            echo "{\"type\":\"init_ok\",\"id\":$id,\"capabilities\":[\"backend\"]}"
            ;;
        activate)
            echo "{\"type\":\"activate_ok\",\"id\":$id}"
            ;;
        deactivate)
            echo "{\"type\":\"deactivate_ok\",\"id\":$id}"
            ;;
        *)
            echo "{\"type\":\"error\",\"id\":$id,\"message\":\"unknown request type: $type\"}"
            ;;
    esac
done
"#;
    let script_path = dir.join("plugin-bin");
    std::fs::write(&script_path, script).expect("write script");
    std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    "plugin-bin".to_string()
}

#[tokio::test]
async fn spawn_nonexistent_entrypoint_errors() {
    let tmp = TempDir::new().expect("tempdir");
    let result = PluginProcess::spawn("test", tmp.path(), "nonexistent").await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, PluginError::ProcessStart(_)),
        "expected ProcessStart, got: {err:?}"
    );
}

#[tokio::test]
async fn spawn_and_init_roundtrip() {
    let tmp = TempDir::new().expect("tempdir");
    let entrypoint = create_echo_plugin(tmp.path());

    let mut proc = PluginProcess::spawn("test-plugin", tmp.path(), &entrypoint)
        .await
        .expect("spawn");

    let id = proc.next_id();
    let request = PluginRequest::Init {
        id,
        jig_version: "0.1.0".to_string(),
        plugin_dir: tmp.path().display().to_string(),
    };

    let response = proc.request(&request).await.expect("request");
    assert_eq!(response.id(), id);
    assert!(
        matches!(response, PluginResponse::InitOk { ref capabilities, .. } if capabilities == &vec!["backend"]),
        "unexpected response: {response:?}"
    );
}

#[tokio::test]
async fn request_activate_roundtrip() {
    let tmp = TempDir::new().expect("tempdir");
    let entrypoint = create_echo_plugin(tmp.path());

    let mut proc = PluginProcess::spawn("test-plugin", tmp.path(), &entrypoint)
        .await
        .expect("spawn");

    let id = proc.next_id();
    let request = PluginRequest::Activate {
        id,
        project_dir: "/tmp/project".to_string(),
    };

    let response = proc.request(&request).await.expect("request");
    assert!(matches!(response, PluginResponse::ActivateOk { .. }));
}

#[tokio::test]
async fn request_deactivate_roundtrip() {
    let tmp = TempDir::new().expect("tempdir");
    let entrypoint = create_echo_plugin(tmp.path());

    let mut proc = PluginProcess::spawn("test-plugin", tmp.path(), &entrypoint)
        .await
        .expect("spawn");

    let id = proc.next_id();
    let request = PluginRequest::Deactivate { id };

    let response = proc.request(&request).await.expect("request");
    assert!(matches!(response, PluginResponse::DeactivateOk { .. }));
}

#[tokio::test]
async fn next_id_increments() {
    let tmp = TempDir::new().expect("tempdir");
    let entrypoint = create_echo_plugin(tmp.path());

    let proc = PluginProcess::spawn("test-plugin", tmp.path(), &entrypoint)
        .await
        .expect("spawn");

    assert_eq!(proc.next_id(), 1);
    assert_eq!(proc.next_id(), 2);
    assert_eq!(proc.next_id(), 3);
}

#[tokio::test]
async fn kill_terminates_process() {
    let tmp = TempDir::new().expect("tempdir");
    let entrypoint = create_echo_plugin(tmp.path());

    let mut proc = PluginProcess::spawn("test-plugin", tmp.path(), &entrypoint)
        .await
        .expect("spawn");

    proc.kill();
    // Give it a moment to die
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    let status = proc.try_wait();
    assert!(status.is_some(), "process should have exited after kill");
}
