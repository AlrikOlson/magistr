use super::*;
use crate::detection::lint::{LintDiagnostic, Severity};

#[test]
fn lint_report_counts() {
    let report = LintReport {
        path: "CLAUDE.md".into(),
        diagnostics: vec![
            LintDiagnostic {
                rule: "a",
                severity: Severity::Error,
                message: "err".into(),
                line: None,
                fix: None,
            },
            LintDiagnostic {
                rule: "b",
                severity: Severity::Warning,
                message: "warn".into(),
                line: None,
                fix: None,
            },
            LintDiagnostic {
                rule: "c",
                severity: Severity::Info,
                message: "info".into(),
                line: None,
                fix: None,
            },
        ],
    };
    assert_eq!(report.errors(), 1);
    assert_eq!(report.warnings(), 1);
    assert!(!report.passed());
}

#[test]
fn lint_report_passes_with_no_errors() {
    let report = LintReport {
        path: "CLAUDE.md".into(),
        diagnostics: vec![LintDiagnostic {
            rule: "a",
            severity: Severity::Warning,
            message: "warn".into(),
            line: None,
            fix: None,
        }],
    };
    assert!(report.passed());
}
