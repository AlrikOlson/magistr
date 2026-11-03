use std::time::Duration;

use super::*;
use crate::utils::check::{CheckRun, CheckVerdict};
use crate::validate::{ProjectValidation as ValidationResult, StandardCheck};

fn make_check_result(check: StandardCheck, passed: bool, output: &str) -> CheckRun {
    CheckRun {
        name: check.label().into(),
        verdict: if passed {
            CheckVerdict::Passed
        } else {
            CheckVerdict::Failed
        },
        required: true,
        exit_code: if passed { 0 } else { 1 },
        rows: vec![],
        queue_duration: std::time::Duration::ZERO,
        duration: Duration::from_millis(100),
        output: output.to_string(),
        command: format!("test-cmd-{}", check.label()),
    }
}

// ── Annotation formatting ──────────────────────────────────

#[test]
fn annotation_to_workflow_command_with_col() {
    let ann = Annotation {
        level: AnnotationLevel::Error,
        file: "src/main.rs".to_string(),
        line: 10,
        col: Some(5),
        message: "unused variable".to_string(),
        title: "lint".to_string(),
    };
    assert_eq!(
        ann.to_workflow_command(),
        "::error file=src/main.rs,line=10,col=5,title=lint::unused variable"
    );
}

#[test]
fn annotation_to_workflow_command_without_col() {
    let ann = Annotation {
        level: AnnotationLevel::Warning,
        file: "src/lib.rs".to_string(),
        line: 42,
        col: None,
        message: "deprecated function".to_string(),
        title: "check".to_string(),
    };
    assert_eq!(
        ann.to_workflow_command(),
        "::warning file=src/lib.rs,line=42,title=check::deprecated function"
    );
}

#[test]
fn notice_level_formats_correctly() {
    let ann = Annotation {
        level: AnnotationLevel::Notice,
        file: "README.md".to_string(),
        line: 1,
        col: None,
        message: "info".to_string(),
        title: "test".to_string(),
    };
    assert!(ann.to_workflow_command().starts_with("::notice "));
}

// ── Rust arrow parser ──────────────────────────────────────

#[test]
fn parse_rust_arrow_with_col() {
    let line = "  --> src/main.rs:10:5";
    let ann = try_parse_rust_arrow(line, "check").unwrap();
    assert_eq!(ann.file, "src/main.rs");
    assert_eq!(ann.line, 10);
    assert_eq!(ann.col, Some(5));
    assert_eq!(ann.level, AnnotationLevel::Error);
    assert_eq!(ann.title, "check");
}

#[test]
fn parse_rust_arrow_without_col() {
    let line = " --> src/lib.rs:42";
    let ann = try_parse_rust_arrow(line, "lint").unwrap();
    assert_eq!(ann.file, "src/lib.rs");
    assert_eq!(ann.line, 42);
    assert_eq!(ann.col, None);
}

#[test]
fn parse_rust_arrow_rejects_non_arrow() {
    assert!(try_parse_rust_arrow("error[E0308]: mismatched types", "check").is_none());
    assert!(try_parse_rust_arrow("  |", "check").is_none());
}

// ── TypeScript parens parser ───────────────────────────────

#[test]
fn parse_ts_error() {
    let line = "src/app.ts(15,3): error TS2322: Type 'string' is not assignable to type 'number'.";
    let ann = try_parse_ts_parens(line, "check").unwrap();
    assert_eq!(ann.file, "src/app.ts");
    assert_eq!(ann.line, 15);
    assert_eq!(ann.col, Some(3));
    assert_eq!(ann.level, AnnotationLevel::Error);
    assert!(ann.message.contains("TS2322"));
}

#[test]
fn parse_ts_warning() {
    let line = "src/util.ts(7,1): warning TS6133: unused import";
    let ann = try_parse_ts_parens(line, "lint").unwrap();
    assert_eq!(ann.level, AnnotationLevel::Warning);
}

#[test]
fn parse_ts_rejects_non_path() {
    // Space in "file" part → not a path
    assert!(try_parse_ts_parens("some function(1,2): error", "check").is_none());
}

// ── Colon-separated parser ─────────────────────────────────

#[test]
fn parse_go_style() {
    let line = "cmd/server.go:42:10: undefined: foo";
    let ann = try_parse_colon_separated(line, "check").unwrap();
    assert_eq!(ann.file, "cmd/server.go");
    assert_eq!(ann.line, 42);
    assert_eq!(ann.col, Some(10));
}

#[test]
fn parse_python_style() {
    let line = "src/main.py:10: error: Incompatible types";
    let ann = try_parse_colon_separated(line, "check").unwrap();
    assert_eq!(ann.file, "src/main.py");
    assert_eq!(ann.line, 10);
    assert_eq!(ann.col, None);
    assert_eq!(ann.level, AnnotationLevel::Error);
}

#[test]
fn parse_eslint_style() {
    let line = "src/index.ts:5:3: warning no-unused-vars";
    let ann = try_parse_colon_separated(line, "lint").unwrap();
    assert_eq!(ann.file, "src/index.ts");
    assert_eq!(ann.line, 5);
    assert_eq!(ann.col, Some(3));
    assert_eq!(ann.level, AnnotationLevel::Warning);
}

#[test]
fn parse_colon_rejects_non_file() {
    // No dot or slash → not a path
    assert!(try_parse_colon_separated("note: some hint", "check").is_none());
    assert!(try_parse_colon_separated("help: try this", "check").is_none());
}

// ── Integration: format_annotations ────────────────────────

#[test]
fn format_annotations_skips_passing_checks() {
    let result = ValidationResult {
        checks: vec![],
        results: vec![make_check_result(StandardCheck::Check, true, "all good")],
    };
    let output = format_annotations(&result);
    assert!(output.is_empty());
}

#[test]
fn format_annotations_emits_general_error_for_unparseable_output() {
    let result = ValidationResult {
        checks: vec![],
        results: vec![make_check_result(
            StandardCheck::Test,
            false,
            "FAILED: 3 tests failed",
        )],
    };
    let output = format_annotations(&result);
    assert!(output.contains("::error title=test::test check failed"));
}

#[test]
fn format_annotations_emits_file_annotations_for_parseable_output() {
    let output_text = "error[E0308]: mismatched types\n  --> src/main.rs:10:5\n  |\n";
    let result = ValidationResult {
        checks: vec![],
        results: vec![make_check_result(StandardCheck::Check, false, output_text)],
    };
    let output = format_annotations(&result);
    assert!(output.contains("::error file=src/main.rs,line=10,col=5,title=check::"));
}

#[test]
fn format_annotations_handles_multiple_failed_checks() {
    let result = ValidationResult {
        checks: vec![],
        results: vec![
            make_check_result(StandardCheck::Check, true, "ok"),
            make_check_result(
                StandardCheck::Test,
                false,
                "src/lib.rs:20: error: assertion failed",
            ),
            make_check_result(StandardCheck::Lint, false, "  --> src/util.rs:5:1\n"),
        ],
    };
    let output = format_annotations(&result);
    // Should have annotations from test and lint, not check
    assert!(output.contains("file=src/lib.rs"));
    assert!(output.contains("file=src/util.rs"));
    assert!(!output.contains("title=check::"));
}

#[test]
fn format_annotations_preserves_custom_and_archetype_check_names() {
    let mut archetype =
        make_check_result(StandardCheck::Check, false, "src/main.rs:8: error: failed");
    archetype.name = "check:website".into();
    let mut custom = make_check_result(StandardCheck::Lint, false, "custom check failed");
    custom.name = "architecture".into();
    let output = format_annotations(&ValidationResult {
        checks: vec![],
        results: vec![archetype, custom],
    });
    assert!(output.contains("title=check%3Awebsite::"));
    assert!(output.contains("::error title=architecture::architecture check failed"));
}

#[test]
fn custom_check_annotations_escape_workflow_command_delimiters() {
    let mut check = make_check_result(StandardCheck::Check, false, "failed");
    check.name = "check:website,1\n::notice::injected".into();
    check.command = "first\r\nsecond%".into();
    let output = format_annotations(&ValidationResult {
        checks: vec![],
        results: vec![check],
    });
    assert_eq!(output.lines().count(), 1);
    assert!(output.contains("title=check%3Awebsite%2C1%0A%3A%3Anotice%3A%3Ainjected::"));
    assert!(output.contains("first%0D%0Asecond%25"));
}
