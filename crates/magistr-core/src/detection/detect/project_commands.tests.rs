use super::*;
use crate::{
    config::preset::{LayerOptions, load_preset_layered},
    types::Framework,
};

fn preset(dir: &Path, manager: PackageManager) -> Preset {
    load_preset_layered(
        Language::TypeScript,
        Framework::None,
        manager,
        &LayerOptions {
            project_root: Some(dir.into()),
            ..Default::default()
        },
    )
    .unwrap()
}

#[test]
fn absent_tooling_keeps_every_check_unconfigured() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("package.json"),
        r#"{"scripts":{"release":"bun build app.ts","smoke":"bun app.ts"}}"#,
    )
    .unwrap();
    let p = preset(
        tmp.path(),
        super::super::detect_package_manager(tmp.path(), Language::TypeScript),
    );
    assert_eq!(p.quality_gate.check, "");
    assert_eq!(p.quality_gate.test, "");
    assert_eq!(p.quality_gate.lint, "");
    assert_eq!(p.format.command, "");
}

#[test]
fn declared_scripts_take_priority_and_missing_scripts_stay_missing() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("package.json"), r#"{"packageManager":"bun@1.3.0","scripts":{"typecheck":"tsc --noEmit","test":"bun test"},"devDependencies":{"vitest":"*","prettier":"*"}}"#).unwrap();
    let p = preset(tmp.path(), PackageManager::Bun);
    assert_eq!(p.quality_gate.check, "bun run typecheck");
    assert_eq!(p.quality_gate.test, "bun run test");
    assert_eq!(p.quality_gate.lint, "");
    assert_eq!(
        p.format.command,
        "./node_modules/.bin/prettier --write {file}"
    );
}

#[test]
fn declared_tools_and_configuration_supply_local_commands() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("package.json"),
        r#"{"devDependencies":{"typescript":"*","vitest":"*","eslint":"*"}}"#,
    )
    .unwrap();
    let without_config = preset(tmp.path(), PackageManager::Unknown);
    assert_eq!(without_config.quality_gate.check, "");
    std::fs::write(tmp.path().join("tsconfig.json"), "{}").unwrap();
    let p = preset(tmp.path(), PackageManager::Unknown);
    assert_eq!(p.quality_gate.check, "./node_modules/.bin/tsc --noEmit");
    assert_eq!(p.quality_gate.test, "./node_modules/.bin/vitest run");
    assert_eq!(p.quality_gate.lint, "./node_modules/.bin/eslint .");
    assert_eq!(p.format.command, "");
}

#[test]
fn explicit_project_and_user_commands_override_observation() {
    let tmp = tempfile::tempdir().unwrap();
    let user = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join(".magistr/presets")).unwrap();
    std::fs::write(
        tmp.path().join(".magistr/presets/checks.json"),
        r#"{"quality_gate":{"test":"bun test"}}"#,
    )
    .unwrap();
    std::fs::write(
        user.path().join("typescript.json"),
        r#"{"quality_gate":{"lint":"custom-lint"},"format":{"command":"custom-format"}}"#,
    )
    .unwrap();
    let p = load_preset_layered(
        Language::TypeScript,
        Framework::None,
        PackageManager::Unknown,
        &LayerOptions {
            project_root: Some(tmp.path().into()),
            user_preset_dir: Some(user.path().into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(p.quality_gate.check, "");
    assert_eq!(p.quality_gate.test, "bun test");
    assert_eq!(p.quality_gate.lint, "custom-lint");
    assert_eq!(p.format.command, "custom-format");
}

#[test]
fn unknown_language_has_no_successful_placeholder_commands() {
    let p = crate::config::preset::fallback_preset();
    assert_eq!(p.quality_gate.check, "");
    assert_eq!(p.quality_gate.test, "");
    assert_eq!(p.quality_gate.lint, "");
}
