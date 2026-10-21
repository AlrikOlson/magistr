use super::*;

use crate::utils::gate::GateVerdict;

fn outcome(name: &str, verdict: GateVerdict, exit_code: i32, duration: Duration) -> GateOutcome {
    GateOutcome {
        name: name.to_owned(),
        command: "cargo nextest run".to_owned(),
        required: true,
        verdict,
        exit_code,
        output: String::new(),
        rows: Vec::new(),
        queue_duration: std::time::Duration::ZERO,
        duration,
    }
}

#[test]
fn evidence_is_the_gate_outcome_verbatim() {
    let evidence = GateEvidence::from(&outcome(
        "test",
        GateVerdict::Failed,
        101,
        Duration::from_millis(12_340),
    ));
    assert_eq!(
        evidence,
        GateEvidence {
            name: "test".to_owned(),
            passed: false,
            duration: Duration::from_millis(12_340),
        }
    );
    assert_eq!(evidence.line(), "- test: FAILED (12.3s)");
}

#[test]
fn a_passed_gate_says_so() {
    let evidence = GateEvidence::from(&outcome(
        "check",
        GateVerdict::Passed,
        0,
        Duration::from_secs(2),
    ));
    assert_eq!(evidence.line(), "- check: PASSED (2.0s)");
}

#[test]
fn a_timed_out_gate_is_not_evidence_that_the_code_is_good() {
    let evidence = GateEvidence::from(&outcome(
        "test",
        GateVerdict::TimedOut,
        -1,
        Duration::from_secs(300),
    ));
    assert!(!evidence.passed);
}
