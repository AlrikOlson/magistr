use std::path::PathBuf;

use super::*;
use crate::types::{Archetype, FormatConfig, ProjectRole, QualityGate, TestingConfig};

fn make_archetype(id: &str, language: &str) -> Archetype {
    Archetype {
        id: id.into(),
        name: id.into(),
        language: language.into(),
        framework: None,
        root: PathBuf::from("."),
        role: ProjectRole::Backend,
        quality_gate: QualityGate {
            check: String::new(),
            test: String::new(),
            lint: String::new(),
        },
        format: FormatConfig {
            extensions: vec![],
            command: String::new(),
            exclusions: vec![],
        },
        testing: TestingConfig {
            companion_pattern: String::new(),
            test_dir: "alongside".into(),
        },
        source_patterns: vec![],
        test_patterns: vec![],
        config_patterns: vec![],
        naming_convention: None,
        module_structure: None,
        model_preference: None,
    }
}

#[test]
fn rust_agent_denies_npm_tools() {
    let denied = language_denied_tools("rust");
    assert!(denied.contains(&"npm".into()));
    assert!(denied.contains(&"npx".into()));
    assert!(denied.contains(&"node".into()));
    // Should not deny cargo.
    assert!(!denied.contains(&"cargo".into()));
}

#[test]
fn typescript_agent_denies_cargo_tools() {
    let denied = language_denied_tools("typescript");
    assert!(denied.contains(&"cargo".into()));
    assert!(denied.contains(&"rustc".into()));
    // Should not deny npm.
    assert!(!denied.contains(&"npm".into()));
}

#[test]
fn unknown_language_denies_nothing() {
    let denied = language_denied_tools("brainfuck");
    assert!(denied.is_empty());
}

#[test]
fn shared_tools_are_recognized() {
    assert!(is_shared_tool("Read"));
    assert!(is_shared_tool("Edit"));
    assert!(is_shared_tool("Bash"));
    assert!(is_shared_tool("mcp__magistr__search"));
    assert!(!is_shared_tool("npm"));
    assert!(!is_shared_tool("cargo"));
}

#[test]
fn apply_policy_adds_denied_tools() {
    let arch = make_archetype("backend", "rust");
    let mut role = crate::role::SpecializedRole::coder();
    assert!(role.denied_tools.is_empty());

    apply_archetype_tool_policy(&mut role, &arch);
    assert!(role.denied_tools.contains(&"npm".into()));
    assert!(role.denied_tools.contains(&"dotnet".into()));
}

#[test]
fn apply_policy_does_not_duplicate_denials() {
    let arch = make_archetype("backend", "rust");
    let mut role = crate::role::SpecializedRole::coder();
    role.denied_tools.push("npm".into());

    apply_archetype_tool_policy(&mut role, &arch);
    let npm_count = role.denied_tools.iter().filter(|t| *t == "npm").count();
    assert_eq!(npm_count, 1);
}

#[test]
fn apply_policy_does_not_deny_shared_tools() {
    let arch = make_archetype("backend", "rust");
    let mut role = crate::role::SpecializedRole::coder();
    apply_archetype_tool_policy(&mut role, &arch);

    // Shared tools must never appear in denied list.
    assert!(!role.denied_tools.contains(&"Read".into()));
    assert!(!role.denied_tools.contains(&"Edit".into()));
    assert!(!role.denied_tools.contains(&"Bash".into()));
}
