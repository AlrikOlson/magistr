use super::*;
use std::path::Path;
use std::time::Duration;

use crate::validate::{Gate, GateResult, ValidationResult};

// ── GateStatus tests ────────────────────────────────────────────────

#[test]
fn gate_status_labels() {
    assert_eq!(GateStatus::Pending.label(), "pending");
    assert_eq!(GateStatus::Running.label(), "running");
    assert_eq!(GateStatus::Passed.label(), "passed");
    assert_eq!(GateStatus::Failed.label(), "failed");
    assert_eq!(GateStatus::Cached.label(), "cached");
}

#[test]
fn gate_status_icons() {
    assert_eq!(GateStatus::Pending.icon(), "○");
    assert_eq!(GateStatus::Running.icon(), "●");
    assert_eq!(GateStatus::Passed.icon(), "✓");
    assert_eq!(GateStatus::Failed.icon(), "✗");
    assert_eq!(GateStatus::Cached.icon(), "◎");
}

#[test]
fn gate_status_terminal() {
    assert!(!GateStatus::Pending.is_terminal());
    assert!(!GateStatus::Running.is_terminal());
    assert!(GateStatus::Passed.is_terminal());
    assert!(GateStatus::Failed.is_terminal());
    assert!(GateStatus::Cached.is_terminal());
}

// ── QualityGateRunner tests ─────────────────────────────────────────

#[tokio::test]
async fn runner_run_all_passes() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();

    let qg = QualityGate {
        check: "true".into(),
        test: "true".into(),
        lint: "true".into(),
    };
    let mut runner = QualityGateRunner::new(qg, dir.path());
    let result = runner.run_all().await.unwrap();

    assert!(result.all_passed());
    assert_eq!(result.results.len(), 3);
}

#[tokio::test]
async fn runner_run_all_with_failures() {
    let dir = tempfile::TempDir::new().unwrap();
    let qg = QualityGate {
        check: "true".into(),
        test: "false".into(),
        lint: "true".into(),
    };
    let mut runner = QualityGateRunner::new(qg, dir.path());
    let result = runner.run_all().await.unwrap();

    assert!(!result.all_passed());
    assert_eq!(result.passed_count(), 2);
    assert_eq!(result.failed_count(), 1);
}

#[tokio::test]
async fn runner_parallel_skips_test_on_check_failure() {
    let dir = tempfile::TempDir::new().unwrap();
    let qg = QualityGate {
        check: "false".into(),
        test: "true".into(), // Would pass, but should be skipped
        lint: "true".into(),
    };
    let runner = QualityGateRunner::new(qg, dir.path());
    let result = runner.run_parallel().await.unwrap();

    // check failed, lint passed, test skipped (failed)
    let check = &result.results[0];
    let lint = &result.results[1];
    let test = &result.results[2];

    assert!(!check.passed);
    assert!(lint.passed);
    assert!(!test.passed);
    assert!(test.output.contains("skipped"));
}

#[tokio::test]
async fn runner_caches_results() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();

    let qg = QualityGate {
        check: "true".into(),
        test: "true".into(),
        lint: "true".into(),
    };
    let mut runner = QualityGateRunner::new(qg, dir.path());

    // First run — not cached
    assert!(!runner.cache_valid());
    let _result1 = runner.run_all().await.unwrap();

    // Second run — should use cache
    assert!(runner.cache_valid());
    let result2 = runner.run_all().await.unwrap();
    assert!(result2.all_passed());
}

#[tokio::test]
async fn runner_invalidates_cache() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();

    let qg = QualityGate {
        check: "true".into(),
        test: "true".into(),
        lint: "true".into(),
    };
    let mut runner = QualityGateRunner::new(qg, dir.path());

    let _result = runner.run_all().await.unwrap();
    assert!(runner.cache_valid());

    runner.invalidate_cache();
    assert!(!runner.cache_valid());
}

// ── Feedback generation tests ───────────────────────────────────────

#[test]
fn feedback_none_when_all_pass() {
    let result = make_validation_result(true, true, true);
    assert!(QualityGateRunner::feedback_for_agent(&result).is_none());
}

#[test]
fn feedback_present_on_failure() {
    let result = make_validation_result(true, false, true);
    let feedback = QualityGateRunner::feedback_for_agent(&result).unwrap();

    assert!(feedback.contains("Quality gate failures detected"));
    assert!(feedback.contains("test"));
    assert!(feedback.contains("❌"));
}

#[test]
fn feedback_includes_failed_gate_output() {
    let mut result = make_validation_result(true, true, true);
    result.results[1].passed = false;
    result.results[1].output = "error[E0308]: mismatched types".into();

    let feedback = QualityGateRunner::feedback_for_agent(&result).unwrap();
    assert!(feedback.contains("mismatched types"));
}

#[test]
fn feedback_truncates_long_output() {
    let long_output = "x".repeat(1000);
    let truncated = truncate_output(&long_output, 500);
    assert_eq!(truncated.len(), 500);
}

#[test]
fn feedback_no_truncation_for_short_output() {
    let short = "hello world";
    assert_eq!(truncate_output(short, 500), short);
}

// ── Runner accessor tests ───────────────────────────────────────────

#[test]
fn runner_accessors() {
    let qg = QualityGate {
        check: "cargo check".into(),
        test: "cargo test".into(),
        lint: "cargo clippy".into(),
    };
    let runner = QualityGateRunner::new(qg.clone(), "/my/project");

    assert_eq!(runner.quality_gate().check, "cargo check");
    assert_eq!(runner.project_dir(), Path::new("/my/project"));
    assert!(runner.last_result().is_none());
    assert!(!runner.cache_valid());
}

// ── Helpers ─────────────────────────────────────────────────────────

fn make_validation_result(check: bool, test: bool, lint: bool) -> ValidationResult {
    ValidationResult {
        results: vec![
            GateResult {
                gate: Gate::Check,
                passed: check,
                duration: Duration::from_secs(1),
                output: if check {
                    "ok".into()
                } else {
                    "check failed".into()
                },
                command: "cargo check".into(),
            },
            GateResult {
                gate: Gate::Test,
                passed: test,
                duration: Duration::from_secs(3),
                output: if test {
                    "ok".into()
                } else {
                    "test failed".into()
                },
                command: "cargo test".into(),
            },
            GateResult {
                gate: Gate::Lint,
                passed: lint,
                duration: Duration::from_secs(2),
                output: if lint {
                    "ok".into()
                } else {
                    "lint failed".into()
                },
                command: "cargo clippy".into(),
            },
        ],
    }
}
