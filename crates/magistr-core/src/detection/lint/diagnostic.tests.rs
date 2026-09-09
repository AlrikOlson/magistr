use super::*;
use crate::detection::lint::Severity;

#[test]
fn lint_diagnostic_display() {
    let d = LintDiagnostic {
        rule: "too-long",
        severity: Severity::Warning,
        message: "File is very long".into(),
        line: Some(501),
        fix: Some("Split into .claude/rules/ files".into()),
    };
    let s = d.to_string();
    assert!(s.contains("[warning]"));
    assert!(s.contains("too-long:501"));
    assert!(s.contains("File is very long"));
    assert!(s.contains("fix: Split into"));
}

#[test]
fn lint_diagnostic_display_no_line() {
    let d = LintDiagnostic {
        rule: "missing-gates",
        severity: Severity::Error,
        message: "No quality gates".into(),
        line: None,
        fix: None,
    };
    let s = d.to_string();
    assert!(s.contains("[error] missing-gates:"));
    assert!(!s.contains("fix:"));
}
