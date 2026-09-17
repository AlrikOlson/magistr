//! Integration tests for plugin system: registry, manifest, detection rules.
//!
//! Tests the full plugin lifecycle: create manifest → install → list → validate → remove.
//! Exercises cross-module interactions between `plugin::registry`, `plugin::types`,
//! and `plugin::loader`.

use std::path::Path;

use tempfile::TempDir;

use magistr::plugin::registry::PluginRegistry;
use magistr::plugin::types::{
    DetectionConfig, MANIFEST_FILENAME, PluginKind, PluginManifest, TemplateConfig,
};

// ── Helpers ────────────────────────────────────────────────────────

fn write_file(dir: &Path, name: &str, content: &str) {
    if let Some(parent) = Path::new(name).parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(dir.join(parent)).unwrap();
    }
    std::fs::write(dir.join(name), content).unwrap();
}

fn create_detection_plugin(dir: &Path) {
    write_file(
        dir,
        MANIFEST_FILENAME,
        r#"
name = "test-detection"
version = "1.0.0"
description = "Test detection plugin"
kind = "detection"
author = "Test Author"
"#,
    );
    write_file(
        dir,
        "detect.toml",
        r#"
[[rules]]
name = "custom-framework"
file_patterns = ["custom.config.js"]
content_patterns = ["@custom/core"]
confidence = 85
category = "framework"
"#,
    );
}

fn create_template_plugin(dir: &Path) {
    write_file(
        dir,
        MANIFEST_FILENAME,
        r#"
name = "test-template"
version = "2.0.0"
description = "Test template plugin"
kind = "template"
"#,
    );
    write_file(
        dir,
        "templates.toml",
        r#"
[[files]]
src = "rules/custom.md"
dest = "rules/custom.md"
"#,
    );
    write_file(
        dir,
        "rules/custom.md",
        "# Custom Rules\n\nDo this, not that.",
    );
}

// ── Plugin install → list → remove lifecycle ───────────────────────

#[test]
fn plugin_install_list_remove_lifecycle() {
    let registry_dir = TempDir::new().unwrap();
    let plugin_dir = TempDir::new().unwrap();

    create_detection_plugin(plugin_dir.path());

    let mut registry = PluginRegistry::new(registry_dir.path());
    assert!(registry.is_empty());

    // Install
    let installed = registry.add_local(plugin_dir.path()).unwrap();
    assert_eq!(installed.manifest.name, "test-detection");
    assert_eq!(installed.manifest.kind, PluginKind::Detection);

    // List
    assert_eq!(registry.len(), 1);
    assert!(registry.is_installed("test-detection"));
    let plugins = registry.list();
    assert_eq!(plugins[0].manifest.version, "1.0.0");

    // Remove
    let removed = registry.remove("test-detection").unwrap();
    assert_eq!(removed.manifest.name, "test-detection");
    assert!(registry.is_empty());
}

// ── Multiple plugins coexist ───────────────────────────────────────

#[test]
fn multiple_plugins_coexist() {
    let registry_dir = TempDir::new().unwrap();
    let detect_dir = TempDir::new().unwrap();
    let template_dir = TempDir::new().unwrap();

    create_detection_plugin(detect_dir.path());
    create_template_plugin(template_dir.path());

    let mut registry = PluginRegistry::new(registry_dir.path());
    registry.add_local(detect_dir.path()).unwrap();
    registry.add_local(template_dir.path()).unwrap();

    assert_eq!(registry.len(), 2);
    assert!(registry.is_installed("test-detection"));
    assert!(registry.is_installed("test-template"));

    // Get by name
    let det = registry
        .list()
        .iter()
        .find(|p| p.manifest.name == "test-detection")
        .unwrap()
        .clone();
    assert_eq!(det.manifest.kind, PluginKind::Detection);

    let tpl = registry
        .list()
        .iter()
        .find(|p| p.manifest.name == "test-template")
        .unwrap()
        .clone();
    assert_eq!(tpl.manifest.kind, PluginKind::Template);
}

// ── Duplicate install fails ────────────────────────────────────────

#[test]
fn duplicate_install_rejected() {
    let registry_dir = TempDir::new().unwrap();
    let plugin_dir = TempDir::new().unwrap();
    create_detection_plugin(plugin_dir.path());

    let mut registry = PluginRegistry::new(registry_dir.path());
    registry.add_local(plugin_dir.path()).unwrap();

    let result = registry.add_local(plugin_dir.path());
    assert!(result.is_err());
}

// ── Manifest parsing validates structure ───────────────────────────

#[test]
fn manifest_parsing_validation() {
    // Valid manifest
    let valid = r#"
name = "my-plugin"
version = "1.0.0"
kind = "detection"
"#;
    let manifest = PluginManifest::from_toml(valid).unwrap();
    assert_eq!(manifest.name, "my-plugin");
    assert_eq!(manifest.kind, PluginKind::Detection);
    assert!(manifest.description.is_none());

    // Invalid manifest (missing required fields)
    let invalid = r#"
name = "incomplete"
"#;
    assert!(PluginManifest::from_toml(invalid).is_err());
}

// ── Detection config parsing ───────────────────────────────────────

#[test]
fn detection_config_parsing() {
    let toml = r#"
[[rules]]
name = "nextjs"
file_patterns = ["next.config.js", "next.config.mjs"]
content_patterns = ["next/app"]
confidence = 90
category = "framework"

[[rules]]
name = "prisma"
file_patterns = ["prisma/schema.prisma"]
confidence = 95
category = "tool"
"#;

    let config = DetectionConfig::from_toml(toml).unwrap();
    assert_eq!(config.rules.len(), 2);
    assert_eq!(config.rules[0].name, "nextjs");
    assert_eq!(config.rules[0].file_patterns.len(), 2);
    assert_eq!(config.rules[0].confidence, 90);
    assert_eq!(config.rules[1].name, "prisma");
    assert!(config.rules[1].content_patterns.is_empty());
}

// ── Template config parsing ────────────────────────────────────────

#[test]
fn template_config_parsing() {
    let toml = r#"
[[files]]
src = "rules/team.md"
dest = "rules/team.md"

[[files]]
src = "hooks/pre-commit.sh"
dest = "hooks/pre-commit.sh"
"#;

    let config = TemplateConfig::from_toml(toml).unwrap();
    assert_eq!(config.files.len(), 2);
    assert_eq!(config.files[0].src, "rules/team.md");
    assert_eq!(config.files[0].dest, "rules/team.md");
}

// ── Registry save/load persistence ─────────────────────────────────

#[test]
fn registry_save_load_round_trip() {
    let registry_dir = TempDir::new().unwrap();
    let plugin_dir = TempDir::new().unwrap();
    create_detection_plugin(plugin_dir.path());

    // Install and save
    {
        let mut registry = PluginRegistry::new(registry_dir.path());
        registry.add_local(plugin_dir.path()).unwrap();
        registry.save().unwrap();
    }

    // Load fresh
    let loaded = PluginRegistry::load(
        registry_dir.path(),
        magistr::plugin::OrgPluginConfig::default(),
    )
    .unwrap();
    assert_eq!(loaded.len(), 1);
    assert!(loaded.is_installed("test-detection"));
}

// ── Remove nonexistent plugin errors ───────────────────────────────

#[test]
fn remove_nonexistent_errors() {
    let registry_dir = TempDir::new().unwrap();
    let mut registry = PluginRegistry::new(registry_dir.path());
    let result = registry.remove("does-not-exist");
    assert!(result.is_err());
}
