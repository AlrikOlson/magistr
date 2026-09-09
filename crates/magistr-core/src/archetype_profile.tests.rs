use std::path::PathBuf;

use super::*;
use crate::types::{Archetype, FormatConfig, ProjectRole, QualityGate, TestingConfig};

fn make_rust_archetype() -> Archetype {
    Archetype {
        id: "backend".into(),
        name: "Rust Backend".into(),
        language: "rust".into(),
        framework: Some("axum".into()),
        root: PathBuf::from("services/api"),
        role: ProjectRole::Backend,
        quality_gate: QualityGate {
            check: "cargo check".into(),
            test: "cargo nextest run".into(),
            lint: "cargo clippy -- -D warnings".into(),
        },
        format: FormatConfig {
            extensions: vec!["rs".into()],
            command: "cargo fmt".into(),
            exclusions: vec![],
        },
        testing: TestingConfig {
            companion_pattern: "{name}.tests.rs".into(),
            test_dir: "alongside".into(),
        },
        source_patterns: vec!["src/**/*.rs".into()],
        test_patterns: vec!["src/**/*.tests.rs".into(), "tests/**/*.rs".into()],
        config_patterns: vec!["Cargo.toml".into()],
        naming_convention: Some("snake_case".into()),
        module_structure: Some("one-pub-per-file".into()),
        model_preference: None,
    }
}

fn make_ts_archetype() -> Archetype {
    Archetype {
        id: "frontend".into(),
        name: "React Frontend".into(),
        language: "typescript".into(),
        framework: Some("nextjs".into()),
        root: PathBuf::from("apps/web"),
        role: ProjectRole::Frontend,
        quality_gate: QualityGate {
            check: "npm run build".into(),
            test: "npm test".into(),
            lint: "npm run lint".into(),
        },
        format: FormatConfig {
            extensions: vec!["ts".into(), "tsx".into()],
            command: "npx prettier --write .".into(),
            exclusions: vec![],
        },
        testing: TestingConfig {
            companion_pattern: "*.test.ts".into(),
            test_dir: "alongside".into(),
        },
        source_patterns: vec!["src/**/*.tsx".into(), "src/**/*.ts".into()],
        test_patterns: vec!["src/**/*.test.ts".into()],
        config_patterns: vec!["package.json".into(), "tsconfig.json".into()],
        naming_convention: Some("camelCase".into()),
        module_structure: Some("barrel-exports".into()),
        model_preference: Some("claude-sonnet-4-6".into()),
    }
}

#[test]
fn archetype_coder_has_archetype_id() {
    let arch = make_rust_archetype();
    let role = archetype_coder(&arch);
    assert_eq!(role.archetype_id.as_deref(), Some("backend"));
    assert_eq!(role.role_id, "coder-backend");
    assert_eq!(role.display_name, "Coder (Rust Backend)");
}

#[test]
fn archetype_coder_includes_conventions() {
    let arch = make_rust_archetype();
    let role = archetype_coder(&arch);
    assert!(role.system_prompt_template.contains("snake_case"));
    assert!(role.system_prompt_template.contains("one-pub-per-file"));
    assert!(role.system_prompt_template.contains("services/api"));
}

#[test]
fn archetype_coder_includes_anti_patterns() {
    let arch = make_rust_archetype();
    let role = archetype_coder(&arch);
    assert!(role.system_prompt_template.contains("No `unwrap()`"));
}

#[test]
fn archetype_coder_includes_framework_patterns() {
    let arch = make_ts_archetype();
    let role = archetype_coder(&arch);
    assert!(role.system_prompt_template.contains("App Router"));
    assert!(role.system_prompt_template.contains("Server Components"));
}

#[test]
fn archetype_coder_uses_archetype_model_preference() {
    let arch = make_ts_archetype();
    let role = archetype_coder(&arch);
    assert_eq!(role.model_preference.as_deref(), Some("claude-sonnet-4-6"));
}

#[test]
fn archetype_coder_falls_back_to_opus() {
    let arch = make_rust_archetype();
    let role = archetype_coder(&arch);
    // No model_preference on the archetype → falls back to opus.
    assert_eq!(role.model_preference.as_deref(), Some("claude-opus-4-6"));
}

#[test]
fn archetype_tester_has_archetype_id() {
    let arch = make_rust_archetype();
    let role = archetype_tester(&arch);
    assert_eq!(role.archetype_id.as_deref(), Some("backend"));
    assert_eq!(role.role_id, "tester-backend");
}

#[test]
fn archetype_tester_includes_test_command() {
    let arch = make_rust_archetype();
    let role = archetype_tester(&arch);
    assert!(role.system_prompt_template.contains("cargo nextest run"));
}

#[test]
fn archetype_tester_includes_test_style() {
    let arch = make_rust_archetype();
    let role = archetype_tester(&arch);
    assert!(role.system_prompt_template.contains("{name}.tests.rs"));
}

#[test]
fn roles_for_archetypes_produces_two_per_archetype() {
    let archetypes = vec![make_rust_archetype(), make_ts_archetype()];
    let roles = roles_for_archetypes(&archetypes);
    assert_eq!(roles.len(), 4);
    assert_eq!(roles[0].role_id, "coder-backend");
    assert_eq!(roles[1].role_id, "tester-backend");
    assert_eq!(roles[2].role_id, "coder-frontend");
    assert_eq!(roles[3].role_id, "tester-frontend");
}

#[test]
fn roles_for_empty_archetypes() {
    let roles = roles_for_archetypes(&[]);
    assert!(roles.is_empty());
}

#[test]
fn render_prompt_substitutes_task() {
    let arch = make_rust_archetype();
    let role = archetype_coder(&arch);
    let rendered = role.render_prompt("Fix the auth middleware");
    assert!(rendered.contains("Fix the auth middleware"));
    assert!(!rendered.contains("{task}"));
}
