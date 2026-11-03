use super::*;

use magistr_core::validate::{StandardCheck, StandardCheckResult};
use std::time::Duration;

fn check(check: StandardCheck, passed: bool, command: &str) -> StandardCheckResult {
    StandardCheckResult {
        check,
        passed,
        duration: Duration::ZERO,
        output: String::new(),
        command: command.to_string(),
    }
}

fn run(results: Vec<StandardCheckResult>) -> ValidationResult {
    ValidationResult { results }
}

fn requiring(checks: &[&str]) -> GovernancePolicy {
    GovernancePolicy {
        required_checks: checks.iter().map(|g| (*g).to_string()).collect(),
        ..GovernancePolicy::default()
    }
}

#[test]
fn a_policy_requiring_nothing_never_complains() {
    let result = run(vec![check(StandardCheck::Test, false, "cargo test")]);
    assert!(check_policy_violations(&GovernancePolicy::default(), &result).is_empty());
}

#[test]
fn a_green_run_satisfies_the_required_checks() {
    let result = run(vec![
        check(StandardCheck::Check, true, "cargo check"),
        check(StandardCheck::Test, true, "cargo nextest run"),
        check(StandardCheck::Lint, true, "cargo clippy"),
    ]);
    assert!(check_policy_violations(&requiring(&["check", "test", "lint"]), &result).is_empty());
}

#[test]
fn a_failing_required_check_is_a_violation() {
    let result = run(vec![
        check(StandardCheck::Check, true, "cargo check"),
        check(StandardCheck::Test, false, "cargo nextest run"),
    ]);
    assert_eq!(
        check_policy_violations(&requiring(&["check", "test"]), &result),
        vec!["test".to_string()]
    );
}

#[test]
fn a_check_the_project_never_configured_does_not_count_as_satisfied() {
    // This is the hole: an unconfigured check reports passed:true.
    let result = run(vec![check(StandardCheck::Test, true, "")]);
    assert_eq!(
        check_policy_violations(&requiring(&["test"]), &result),
        vec!["test".to_string()]
    );
}

#[test]
fn a_required_check_the_run_never_produced_is_a_violation() {
    let result = run(vec![check(StandardCheck::Check, true, "cargo check")]);
    assert_eq!(
        check_policy_violations(&requiring(&["check", "test"]), &result),
        vec!["test".to_string()]
    );
}

#[test]
fn the_roster_can_permit_a_bypass() {
    let policy = GovernancePolicy {
        required_checks: vec!["test".into()],
        allow_check_bypass: true,
        ..GovernancePolicy::default()
    };
    let result = run(vec![check(StandardCheck::Test, false, "cargo nextest run")]);
    assert!(check_policy_violations(&policy, &result).is_empty());
}
