use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use tempfile::TempDir;

use super::*;
use crate::plugin::types::{DiscoveredPlugin, PluginKind, PluginManifest, PluginScope};

fn create_mock_plugin(tmp: &TempDir) -> DiscoveredPlugin {
    let script = r#"#!/bin/sh
while IFS= read -r line; do
    id=$(echo "$line" | sed -n 's/.*"id":\([0-9]*\).*/\1/p')
    type=$(echo "$line" | sed -n 's/.*"type":"\([^"]*\)".*/\1/p')
    case "$type" in
        init)
            echo "{\"type\":\"init_ok\",\"id\":$id,\"capabilities\":[\"step\"]}"
            ;;
        activate)
            echo "{\"type\":\"activate_ok\",\"id\":$id}"
            ;;
        deactivate)
            echo "{\"type\":\"deactivate_ok\",\"id\":$id}"
            ;;
        *)
            echo "{\"type\":\"error\",\"id\":$id,\"message\":\"unknown\"}"
            ;;
    esac
done
"#;
    let script_path = tmp.path().join("plugin-bin");
    std::fs::write(&script_path, script).expect("write script");
    std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755)).expect("chmod");

    DiscoveredPlugin {
        manifest: PluginManifest {
            name: "test-step".to_string(),
            version: "0.1.0".to_string(),
            description: Some("Test step plugin".to_string()),
            kind: PluginKind::WorkflowStep,
            author: None,
            entrypoint: Some("plugin-bin".to_string()),
            permissions: Vec::new(),
            dependencies: Vec::new(),
            compatibility: Default::default(),
        },
        path: tmp.path().to_path_buf(),
        scope: PluginScope::ProjectLocal,
    }
}

#[tokio::test]
async fn full_lifecycle_spawn_init_activate_deactivate() {
    let tmp = TempDir::new().expect("tempdir");
    let plugin = create_mock_plugin(&tmp);

    let mut instance = PluginInstance::spawn(plugin).await.expect("spawn");
    assert_eq!(instance.state(), PluginState::Spawned);

    let caps = instance.init("0.1.0").await.expect("init");
    assert_eq!(caps, &["step"]);
    assert_eq!(instance.state(), PluginState::Initialized);
    assert!(instance.has_capability("step"));

    instance
        .activate(std::path::Path::new("/tmp/project"))
        .await
        .expect("activate");
    assert_eq!(instance.state(), PluginState::Active);

    instance.deactivate().await.expect("deactivate");
    assert_eq!(instance.state(), PluginState::Deactivated);
}

#[tokio::test]
async fn init_requires_spawned_state() {
    let tmp = TempDir::new().expect("tempdir");
    let plugin = create_mock_plugin(&tmp);

    let mut instance = PluginInstance::spawn(plugin).await.expect("spawn");
    instance.init("0.1.0").await.expect("init");

    // Double init should fail
    let err = instance.init("0.1.0").await.unwrap_err();
    assert!(matches!(err, PluginError::LifecycleViolation(_)));
}

#[tokio::test]
async fn activate_requires_initialized_state() {
    let tmp = TempDir::new().expect("tempdir");
    let plugin = create_mock_plugin(&tmp);

    let mut instance = PluginInstance::spawn(plugin).await.expect("spawn");

    // Activate before init should fail
    let err = instance
        .activate(std::path::Path::new("/tmp"))
        .await
        .unwrap_err();
    assert!(matches!(err, PluginError::LifecycleViolation(_)));
}

#[tokio::test]
async fn deactivate_requires_active_or_initialized() {
    let tmp = TempDir::new().expect("tempdir");
    let plugin = create_mock_plugin(&tmp);

    let mut instance = PluginInstance::spawn(plugin).await.expect("spawn");

    // Deactivate in spawned state should fail
    let err = instance.deactivate().await.unwrap_err();
    assert!(matches!(err, PluginError::LifecycleViolation(_)));
}

#[tokio::test]
async fn deactivate_from_initialized_state_works() {
    let tmp = TempDir::new().expect("tempdir");
    let plugin = create_mock_plugin(&tmp);

    let mut instance = PluginInstance::spawn(plugin).await.expect("spawn");
    instance.init("0.1.0").await.expect("init");
    instance.deactivate().await.expect("deactivate");
    assert_eq!(instance.state(), PluginState::Deactivated);
}

#[tokio::test]
async fn spawn_without_entrypoint_errors() {
    let plugin = DiscoveredPlugin {
        manifest: PluginManifest {
            name: "no-entry".to_string(),
            version: "0.1.0".to_string(),
            description: None,
            kind: PluginKind::Detection, // declarative — no entrypoint
            author: None,
            entrypoint: None,
            permissions: Vec::new(),
            dependencies: Vec::new(),
            compatibility: Default::default(),
        },
        path: PathBuf::from("/tmp/nonexistent"),
        scope: PluginScope::Global,
    };

    let err = PluginInstance::spawn(plugin).await.unwrap_err();
    assert!(matches!(err, PluginError::LifecycleViolation(_)));
}

#[tokio::test]
async fn kill_sets_deactivated_state() {
    let tmp = TempDir::new().expect("tempdir");
    let plugin = create_mock_plugin(&tmp);

    let mut instance = PluginInstance::spawn(plugin).await.expect("spawn");
    instance.kill();
    assert_eq!(instance.state(), PluginState::Deactivated);
}

#[tokio::test]
async fn manifest_accessor() {
    let tmp = TempDir::new().expect("tempdir");
    let plugin = create_mock_plugin(&tmp);

    let instance = PluginInstance::spawn(plugin).await.expect("spawn");
    assert_eq!(instance.manifest().name, "test-step");
    assert_eq!(instance.manifest().kind, PluginKind::WorkflowStep);
}
