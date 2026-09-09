use super::*;

use magistr_core::validate::{Gate, GateResult};
use std::time::Duration;

fn gate(gate: Gate, passed: bool, command: &str) -> GateResult {
    GateResult {
        gate,
        passed,
        duration: Duration::ZERO,
        output: String::new(),
        command: command.to_string(),
    }
}

fn run(results: Vec<GateResult>) -> ValidationResult {
    ValidationResult { results }
}

fn requiring(gates: &[&str]) -> GovernancePolicy {
    GovernancePolicy {
        required_gates: gates.iter().map(|g| (*g).to_string()).collect(),
        ..GovernancePolicy::default()
    }
}

#[test]
fn a_policy_requiring_nothing_never_complains() {
    let result = run(vec![gate(Gate::Test, false, "cargo test")]);
    assert!(gate_policy_violations(&GovernancePolicy::default(), &result).is_empty());
}

#[test]
fn a_green_run_satisfies_the_required_gates() {
    let result = run(vec![
        gate(Gate::Check, true, "cargo check"),
        gate(Gate::Test, true, "cargo nextest run"),
        gate(Gate::Lint, true, "cargo clippy"),
    ]);
    assert!(gate_policy_violations(&requiring(&["check", "test", "lint"]), &result).is_empty());
}

#[test]
fn a_failing_required_gate_is_a_violation() {
    let result = run(vec![
        gate(Gate::Check, true, "cargo check"),
        gate(Gate::Test, false, "cargo nextest run"),
    ]);
    assert_eq!(
        gate_policy_violations(&requiring(&["check", "test"]), &result),
        vec!["test".to_string()]
    );
}

#[test]
fn a_gate_the_project_never_configured_does_not_count_as_satisfied() {
    // This is the hole: an unconfigured gate reports passed:true.
    let result = run(vec![gate(Gate::Test, true, "")]);
    assert_eq!(
        gate_policy_violations(&requiring(&["test"]), &result),
        vec!["test".to_string()]
    );
}

#[test]
fn a_required_gate_the_run_never_produced_is_a_violation() {
    let result = run(vec![gate(Gate::Check, true, "cargo check")]);
    assert_eq!(
        gate_policy_violations(&requiring(&["check", "test"]), &result),
        vec!["test".to_string()]
    );
}

#[test]
fn the_roster_can_permit_a_bypass() {
    let policy = GovernancePolicy {
        required_gates: vec!["test".into()],
        allow_gate_bypass: true,
        ..GovernancePolicy::default()
    };
    let result = run(vec![gate(Gate::Test, false, "cargo nextest run")]);
    assert!(gate_policy_violations(&policy, &result).is_empty());
}
