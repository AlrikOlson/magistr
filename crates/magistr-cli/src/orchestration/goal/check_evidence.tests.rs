use super::*;

use crate::utils::check::CheckVerdict;

fn outcome(name: &str, verdict: CheckVerdict, exit_code: i32, duration: Duration) -> CheckRun {
    CheckRun {
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
fn evidence_is_the_check_outcome_verbatim() {
    let evidence = CheckEvidence::from(&outcome(
        "test",
        CheckVerdict::Failed,
        101,
        Duration::from_millis(12_340),
    ));
    assert_eq!(
        evidence,
        CheckEvidence {
            name: "test".to_owned(),
            passed: false,
            duration: Duration::from_millis(12_340),
        }
    );
    assert_eq!(evidence.line(), "- test: FAILED (12.3s)");
}

#[test]
fn a_passed_check_says_so() {
    let evidence = CheckEvidence::from(&outcome(
        "check",
        CheckVerdict::Passed,
        0,
        Duration::from_secs(2),
    ));
    assert_eq!(evidence.line(), "- check: PASSED (2.0s)");
}

#[test]
fn a_timed_out_check_is_not_evidence_that_the_code_is_good() {
    let evidence = CheckEvidence::from(&outcome(
        "test",
        CheckVerdict::TimedOut,
        -1,
        Duration::from_secs(300),
    ));
    assert!(!evidence.passed);
}
