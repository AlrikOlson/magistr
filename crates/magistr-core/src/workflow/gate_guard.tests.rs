use super::*;
use std::time::Duration;

use crate::validate::{Gate, GateResult, ValidationResult};
use crate::workflow::guards::{GuardAction, GuardContext};

#[test]
fn guard_name() {
    let guard = QualityGateGuard::new();
    assert_eq!(guard.name(), "quality_gate");
}

#[test]
fn guard_steers_when_no_results() {
    let guard = QualityGateGuard::new();
    let ctx = make_guard_context();

    let result = guard.pre_commit(&ctx);
    assert!(!result.is_ok());
    assert!(matches!(result.action, GuardAction::SteerPrompt(ref s) if s.contains("not been run")));
}

#[test]
fn guard_ok_when_all_pass() {
    let mut guard = QualityGateGuard::new();
    guard.set_last_result(make_validation_result(true, true, true));

    let ctx = make_guard_context();
    let result = guard.pre_commit(&ctx);
    assert!(result.is_ok());
}

#[test]
fn guard_steers_on_failure() {
    let mut guard = QualityGateGuard::new();
    guard.set_last_result(make_validation_result(true, false, true));

    let ctx = make_guard_context();
    let result = guard.pre_commit(&ctx);
    assert!(!result.is_ok());
    assert!(matches!(result.action, GuardAction::SteerPrompt(ref s) if s.contains("Quality gate")));
}

#[test]
fn guard_clear_resets_to_no_results() {
    let mut guard = QualityGateGuard::new();
    guard.set_last_result(make_validation_result(true, true, true));

    let ctx = make_guard_context();
    assert!(guard.pre_commit(&ctx).is_ok());

    guard.clear();
    assert!(!guard.pre_commit(&ctx).is_ok());
}

#[test]
fn guard_default_hooks_are_ok() {
    let guard = QualityGateGuard::new();
    let ctx = make_guard_context();
    assert!(guard.pre_session(&ctx).is_ok());
    assert!(guard.post_session(&ctx).is_ok());
    assert!(guard.post_commit(&ctx).is_ok());
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

fn make_guard_context() -> GuardContext {
    GuardContext::new(
        "/tmp/project",
        "/tmp/project/ROADMAP.md",
        magistr_engine::ShipGuardState::default(),
    )
}
