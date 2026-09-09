use super::*;
use crate::types::ProjectConfig;

fn config(language: &str, framework: &str) -> ProjectConfig {
    serde_json::from_value(serde_json::json!({
        "project": { "name": "magistr", "description": "", "type": "cli" },
        "stack": {
            "language": language, "framework": framework,
            "package_manager": "cargo", "test_framework": "cargo-test",
            "linter": "clippy", "formatter": "rustfmt"
        },
        "quality_gate": { "check": "", "test": "", "lint": "" },
        "format": { "extensions": ["rs"], "command": "cargo fmt" },
        "spec": { "file": "ROADMAP.md", "unchecked_pattern": "" },
        "testing": { "companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside" },
        "reference_repos": [],
        "state_prefix": "magistr"
    }))
    .expect("project config")
}

#[test]
fn stack_from_a_configured_project_is_its_declared_stack() {
    let stack = Stack::from_project_config(&config("rust", "none")).expect("stack");
    assert_eq!(stack.project_name, "magistr");
    assert_eq!(stack.language, Language::Rust);
    assert_eq!(stack.framework, Framework::None);
    assert_eq!(stack.package_manager, PackageManager::Cargo);
    assert_eq!(stack.formatter, Formatter::Rustfmt);
}

#[test]
fn an_unknown_language_name_yields_no_stack_rather_than_a_wrong_one() {
    assert!(Stack::from_project_config(&config("brainfuck", "none")).is_none());
}
