use std::path::Path;

use tempfile::TempDir;

use super::*;
use crate::plugin::types::PluginKind;

fn write_manifest(dir: &Path, name: &str, kind: &str) {
    std::fs::create_dir_all(dir).expect("create plugin dir");
    let manifest = format!(
        r#"name = "{name}"
version = "0.1.0"
kind = "{kind}"
description = "Test plugin"
"#
    );
    std::fs::write(dir.join("magistr-plugin.toml"), manifest).expect("write manifest");
}

fn write_executable_manifest(dir: &Path, name: &str, kind: &str) {
    std::fs::create_dir_all(dir).expect("create plugin dir");
    let manifest = format!(
        r#"name = "{name}"
version = "0.1.0"
kind = "{kind}"
description = "Test executable plugin"
entrypoint = "plugin-bin"
"#
    );
    std::fs::write(dir.join("magistr-plugin.toml"), manifest).expect("write manifest");
    std::fs::write(dir.join("plugin-bin"), "#!/bin/sh\necho ok").expect("write entrypoint");
}

#[test]
fn discover_empty_project_returns_empty() {
    let tmp = TempDir::new().expect("tempdir");
    let plugins = discover_plugins(tmp.path());
    assert!(plugins.is_empty());
}

#[test]
fn discover_local_plugins_found() {
    let tmp = TempDir::new().expect("tempdir");
    let plugin_dir = tmp
        .path()
        .join(".magistr")
        .join("plugins")
        .join("my-detect");
    write_manifest(&plugin_dir, "my-detect", "detection");

    let plugins = discover_plugins(tmp.path());
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].manifest.name, "my-detect");
    assert_eq!(plugins[0].manifest.kind, PluginKind::Detection);
    assert_eq!(plugins[0].scope, PluginScope::ProjectLocal);
}

#[test]
fn discover_multiple_local_plugins() {
    let tmp = TempDir::new().expect("tempdir");
    let base = tmp.path().join(".magistr").join("plugins");

    write_manifest(&base.join("plugin-a"), "plugin-a", "detection");
    write_manifest(&base.join("plugin-b"), "plugin-b", "template");

    let plugins = discover_plugins(tmp.path());
    assert_eq!(plugins.len(), 2);

    let names: Vec<&str> = plugins.iter().map(|p| p.manifest.name.as_str()).collect();
    assert!(names.contains(&"plugin-a"));
    assert!(names.contains(&"plugin-b"));
}

#[test]
fn discover_skips_invalid_manifests() {
    let tmp = TempDir::new().expect("tempdir");
    let base = tmp.path().join(".magistr").join("plugins");

    // Valid plugin
    write_manifest(&base.join("good"), "good", "detection");

    // Invalid plugin (bad TOML)
    let bad_dir = base.join("bad");
    std::fs::create_dir_all(&bad_dir).expect("create dir");
    std::fs::write(bad_dir.join("magistr-plugin.toml"), "not valid toml {{{{").expect("write");

    let plugins = discover_plugins(tmp.path());
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].manifest.name, "good");
}

#[test]
fn discover_skips_non_directories() {
    let tmp = TempDir::new().expect("tempdir");
    let base = tmp.path().join(".magistr").join("plugins");
    std::fs::create_dir_all(&base).expect("create dir");

    // A file, not a directory — should be skipped
    std::fs::write(base.join("not-a-plugin"), "hello").expect("write");

    write_manifest(&base.join("real"), "real", "detection");

    let plugins = discover_plugins(tmp.path());
    assert_eq!(plugins.len(), 1);
}

#[test]
fn local_plugins_dir_resolves_correctly() {
    let dir = local_plugins_dir(Path::new("/home/user/project"));
    assert_eq!(dir, Path::new("/home/user/project/.magistr/plugins"));
}

#[test]
fn discover_executable_plugin() {
    let tmp = TempDir::new().expect("tempdir");
    let plugin_dir = tmp
        .path()
        .join(".magistr")
        .join("plugins")
        .join("my-backend");
    write_executable_manifest(&plugin_dir, "my-gate", "quality-gate");

    let plugins = discover_plugins(tmp.path());
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].manifest.kind, PluginKind::QualityGate);
    assert!(plugins[0].manifest.kind.is_executable());
    assert_eq!(
        plugins[0].manifest.entrypoint.as_deref(),
        Some("plugin-bin")
    );
}
