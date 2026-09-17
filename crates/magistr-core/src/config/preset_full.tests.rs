use super::*;

use crate::types::{Framework, Language, PackageManager};

// No fixture names a user preset directory, so a developer's HOME or
// XDG_CONFIG_HOME cannot replace the built-in commands under test.
fn layered(
    lang: Language,
    framework: Framework,
    package_manager: PackageManager,
    options: &LayerOptions,
) -> Result<Preset> {
    load_preset_layered(lang, framework, package_manager, options)
}

#[test]
fn language_preset_without_framework_keeps_rust_commands() {
    let preset = layered(
        Language::Rust,
        Framework::None,
        PackageManager::Cargo,
        &LayerOptions::default(),
    )
    .unwrap();
    assert!(preset.quality_gate.check.contains("cargo check"));
}

#[test]
fn framework_layer_replaces_the_language_commands() {
    let preset = layered(
        Language::TypeScript,
        Framework::Next,
        PackageManager::Npm,
        &LayerOptions::default(),
    )
    .unwrap();
    assert!(preset.quality_gate.check.contains("next lint"));
}

#[test]
fn package_manager_layer_replaces_the_language_commands() {
    let preset = layered(
        Language::Java,
        Framework::None,
        PackageManager::Gradle,
        &LayerOptions::default(),
    )
    .unwrap();
    assert!(preset.quality_gate.check.contains("gradlew"));
}

// ── Extended layered pipeline ───────────────────────────────────

#[test]
fn layered_with_database() {
    let preset = layered(
        Language::Rust,
        Framework::None,
        PackageManager::Cargo,
        &LayerOptions {
            database: Some("diesel".to_string()),
            ..Default::default()
        },
    )
    .unwrap();

    assert!(preset.allowed_tools.iter().any(|t| t.contains("diesel")));
    // Base tools preserved
    assert!(preset.allowed_tools.contains(&"Read".to_string()));
}

#[test]
fn layered_with_ci() {
    let preset = layered(
        Language::TypeScript,
        Framework::None,
        PackageManager::Npm,
        &LayerOptions {
            ci: Some("github-actions".to_string()),
            ..Default::default()
        },
    )
    .unwrap();

    assert!(preset.allowed_tools.iter().any(|t| t.contains("gh")));
}

#[test]
fn layered_with_deployment() {
    let preset = layered(
        Language::Python,
        Framework::None,
        PackageManager::Pip,
        &LayerOptions {
            deployment: Some("docker".to_string()),
            ..Default::default()
        },
    )
    .unwrap();

    assert!(preset.allowed_tools.iter().any(|t| t.contains("docker")));
    assert!(preset.format.exclusions.contains(&".docker".to_string()));
}

#[test]
fn layered_all_layers_combined() {
    let preset = layered(
        Language::Rust,
        Framework::Axum,
        PackageManager::Cargo,
        &LayerOptions {
            database: Some("sqlx".to_string()),
            ci: Some("github-actions".to_string()),
            deployment: Some("docker".to_string()),
            ..Default::default()
        },
    )
    .unwrap();

    // Framework applied
    assert!(preset.quality_gate.check.contains("--all-targets"));
    // Database tools added
    assert!(preset.allowed_tools.iter().any(|t| t.contains("sqlx")));
    // CI tools added
    assert!(preset.allowed_tools.iter().any(|t| t.contains("gh")));
    // Deployment tools added
    assert!(preset.allowed_tools.iter().any(|t| t.contains("docker")));
}

#[test]
fn layered_unknown_database_slug_skipped() {
    let preset = layered(
        Language::Rust,
        Framework::None,
        PackageManager::Cargo,
        &LayerOptions {
            database: Some("cassandra-unknown".to_string()),
            ..Default::default()
        },
    )
    .unwrap();

    // Should still work — unknown slug is gracefully skipped
    assert!(preset.quality_gate.check.contains("cargo check"));
}

#[test]
fn layered_with_project_local() {
    let root = tempfile::TempDir::new().unwrap();
    let preset_dir = root.path().join(".magistr/presets");
    std::fs::create_dir_all(&preset_dir).unwrap();
    std::fs::write(
        preset_dir.join("custom.json"),
        r#"{"extra_allowed_tools": ["Bash(custom-tool:*)"]}"#,
    )
    .unwrap();

    let preset = layered(
        Language::Rust,
        Framework::None,
        PackageManager::Cargo,
        &LayerOptions {
            project_root: Some(root.path().into()),
            ..Default::default()
        },
    )
    .unwrap();

    assert!(
        preset
            .allowed_tools
            .iter()
            .any(|t| t.contains("custom-tool"))
    );
}

#[test]
fn the_supplied_user_override_is_the_final_layer() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("rust.json"),
        r#"{
            "quality_gate": { "check": "user-specific check" },
            "extra_allowed_tools": ["Bash(user-tool:*)"]
        }"#,
    )
    .unwrap();
    let preset = load_preset_layered(
        Language::Rust,
        Framework::Axum,
        PackageManager::Cargo,
        &LayerOptions {
            user_preset_dir: Some(dir.path().into()),
            ..LayerOptions::default()
        },
    )
    .unwrap();

    assert_eq!(preset.quality_gate.check, "user-specific check");
    assert!(
        preset
            .allowed_tools
            .contains(&"Bash(user-tool:*)".to_string())
    );
    assert!(preset.allowed_tools.contains(&"Read".to_string()));
}

#[test]
fn a_user_override_load_error_is_preserved() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("rust.json"), "not valid json").unwrap();
    let err = load_preset_layered(
        Language::Rust,
        Framework::None,
        PackageManager::Cargo,
        &LayerOptions {
            user_preset_dir: Some(dir.path().into()),
            ..LayerOptions::default()
        },
    )
    .unwrap_err();

    assert!(err.to_string().contains("invalid user preset at"), "{err}");
}
