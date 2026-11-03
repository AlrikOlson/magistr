use super::*;
use std::time::Duration;

use crate::validate::{StandardCheck, StandardCheckResult, ValidationResult};
use crate::workflow::guards::{GuardAction, GuardContext};

#[test]
fn guard_name() {
    let guard = CheckGuard::new();
    assert_eq!(guard.name(), "checks");
}

#[test]
fn guard_steers_when_no_results() {
    let guard = CheckGuard::new();
    let ctx = make_guard_context();

    let result = guard.pre_commit(&ctx);
    assert!(!result.is_ok());
    assert!(matches!(result.action, GuardAction::SteerPrompt(ref s) if s.contains("not been run")));
}

#[test]
fn guard_ok_when_all_pass() {
    let mut guard = CheckGuard::new();
    guard.set_last_result(make_validation_result(true, true, true));

    let ctx = make_guard_context();
    let result = guard.pre_commit(&ctx);
    assert!(result.is_ok());
}

#[test]
fn guard_steers_on_failure() {
    let mut guard = CheckGuard::new();
    guard.set_last_result(make_validation_result(true, false, true));

    let ctx = make_guard_context();
    let result = guard.pre_commit(&ctx);
    assert!(!result.is_ok());
    assert!(matches!(result.action, GuardAction::SteerPrompt(ref s) if s.contains("Check")));
}

#[test]
fn guard_clear_resets_to_no_results() {
    let mut guard = CheckGuard::new();
    guard.set_last_result(make_validation_result(true, true, true));

    let ctx = make_guard_context();
    assert!(guard.pre_commit(&ctx).is_ok());

    guard.clear();
    assert!(!guard.pre_commit(&ctx).is_ok());
}

#[test]
fn guard_default_hooks_are_ok() {
    let guard = CheckGuard::new();
    let ctx = make_guard_context();
    assert!(guard.pre_session(&ctx).is_ok());
    assert!(guard.post_session(&ctx).is_ok());
    assert!(guard.post_commit(&ctx).is_ok());
}

// ── Helpers ─────────────────────────────────────────────────────────

fn make_validation_result(check: bool, test: bool, lint: bool) -> ValidationResult {
    ValidationResult {
        results: vec![
            StandardCheckResult {
                check: StandardCheck::Check,
                passed: check,
                duration: Duration::from_secs(1),
                output: if check {
                    "ok".into()
                } else {
                    "check failed".into()
                },
                command: "cargo check".into(),
            },
            StandardCheckResult {
                check: StandardCheck::Test,
                passed: test,
                duration: Duration::from_secs(3),
                output: if test {
                    "ok".into()
                } else {
                    "test failed".into()
                },
                command: "cargo test".into(),
            },
            StandardCheckResult {
                check: StandardCheck::Lint,
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
        "/tmp/project/PLAN.md",
        magistr_engine::RunGuardState::default(),
    )
}
