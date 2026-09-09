use std::path::{Path, PathBuf};

use super::super::rules::AutoAllowRule;
use super::*;
use tempfile::TempDir;

// ── PermissionConfig ───────────────────────────────────────────

#[test]
fn default_config_is_standard() {
    let config = PermissionConfig::default();
    assert_eq!(config.profile, PermissionProfile::Standard);
    assert!(config.rules.is_empty());
}

#[test]
fn with_profile_sets_profile() {
    let config = PermissionConfig::with_profile(PermissionProfile::Trusted);
    assert_eq!(config.profile, PermissionProfile::Trusted);
}

#[test]
fn add_rule_appends() {
    let mut config = PermissionConfig::default();
    config.add_rule(AutoAllowRule::all_reads("allow reads"));
    assert_eq!(config.rules.len(), 1);
}

// ── TOML serialization ─────────────────────────────────────────

#[test]
fn toml_round_trip_default() {
    let config = PermissionConfig::default();
    let toml = config.to_toml().unwrap();
    let back = PermissionConfig::from_toml(&toml).unwrap();
    assert_eq!(config.profile, back.profile);
}

#[test]
fn toml_round_trip_with_rules() {
    let mut config = PermissionConfig::with_profile(PermissionProfile::Trusted);
    config.add_rule(AutoAllowRule::all_reads("allow reads in project"));
    config.add_rule(AutoAllowRule::bash_prefix("cargo ", "allow cargo commands"));

    let toml = config.to_toml().unwrap();
    let back = PermissionConfig::from_toml(&toml).unwrap();
    assert_eq!(back.profile, PermissionProfile::Trusted);
    assert_eq!(back.rules.len(), 2);
}

#[test]
fn toml_output_readable() {
    let config = PermissionConfig::with_profile(PermissionProfile::Cautious);
    let toml = config.to_toml().unwrap();
    assert!(
        toml.contains("cautious"),
        "TOML should contain profile name"
    );
}

#[test]
fn toml_empty_rules_omitted() {
    let config = PermissionConfig::default();
    let toml = config.to_toml().unwrap();
    assert!(!toml.contains("[[rules]]"), "Empty rules should be omitted");
}

#[test]
fn from_toml_invalid_returns_error() {
    let result = PermissionConfig::from_toml("not valid toml {{{}}}");
    assert!(result.is_err());
}

#[test]
fn from_toml_partial_uses_defaults() {
    let config = PermissionConfig::from_toml("").unwrap();
    assert_eq!(config.profile, PermissionProfile::Standard);
}

// ── File persistence ───────────────────────────────────────────

#[test]
fn save_and_load_round_trip() {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join(".claude")).unwrap();

    let mut config = PermissionConfig::with_profile(PermissionProfile::Yolo);
    config.add_rule(AutoAllowRule::all_reads("allow reads"));

    save_config(dir.path(), &config).unwrap();
    let loaded = load_config(dir.path()).unwrap();

    assert_eq!(loaded.profile, PermissionProfile::Yolo);
    assert_eq!(loaded.rules.len(), 1);
}

#[test]
fn load_missing_returns_default() {
    let dir = TempDir::new().unwrap();
    let config = load_config(dir.path()).unwrap();
    assert_eq!(config.profile, PermissionProfile::Standard);
}

#[test]
fn save_creates_directories() {
    let dir = TempDir::new().unwrap();
    // Don't create .claude/ first
    let config = PermissionConfig::default();
    save_config(dir.path(), &config).unwrap();

    assert!(config_path(dir.path()).exists());
}

#[test]
fn config_path_location() {
    let path = config_path(Path::new("/my/project"));
    assert_eq!(path, PathBuf::from("/my/project/.claude/permissions.toml"));
}
