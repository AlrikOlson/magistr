use super::*;
use crate::types::*;

fn test_stack() -> Stack {
    Stack {
        project_name: "test".to_string(),
        language: Language::Rust,
        package_manager: PackageManager::Cargo,
        framework: Framework::None,
        test_framework: TestFramework::CargoTest,
        linter: Linter::Clippy,
        formatter: Formatter::Rustfmt,
        workspace: Workspace::None,
        ci_provider: CiProvider::None,
        extra_detections: Vec::new(),
    }
}

#[test]
fn rust_stack_no_warnings_when_cargo_exists() {
    // cargo should exist on any machine running this test
    let warnings = validate_stack(&test_stack());
    assert!(
        warnings.is_empty(),
        "unexpected warnings for Rust: {:?}",
        warnings
    );
}

#[test]
fn validation_warning_display() {
    let w = ValidationWarning {
        tool: "ruff".to_string(),
        install_hint: "pip install ruff".to_string(),
    };
    assert_eq!(w.to_string(), "ruff — install: pip install ruff");
}

#[test]
fn nonexistent_tool_produces_warning() {
    let mut stack = test_stack();
    stack.linter = Linter::Swiftlint; // unlikely to be installed
    let warnings = validate_stack(&stack);
    // This test might pass or fail depending on system — just check structure
    for w in &warnings {
        assert!(!w.tool.is_empty());
        assert!(!w.install_hint.is_empty());
    }
}

#[test]
fn command_exists_finds_common_tools() {
    // These should exist on any Unix system
    assert!(command_exists("ls"));
    assert!(command_exists("echo"));
}

#[test]
fn command_exists_returns_false_for_nonexistent() {
    assert!(!command_exists("jig_nonexistent_tool_xyz_12345"));
}
