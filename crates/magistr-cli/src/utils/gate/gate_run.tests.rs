use super::*;
use std::time::Duration;

fn run(verdict: GateVerdict, exit_code: i32) -> GateRun {
    GateRun {
        name: "test".to_owned(),
        command: "cargo nextest run".to_owned(),
        required: true,
        verdict,
        exit_code,
        output: String::new(),
        rows: Vec::new(),
        queue_duration: std::time::Duration::ZERO,
        duration: Duration::from_secs(2),
    }
}

#[test]
fn a_run_carries_what_the_command_actually_did() {
    let gate = run(GateVerdict::Failed, 101);
    assert_eq!(gate.exit_code, 101);
    assert_eq!(gate.command, "cargo nextest run");
    assert!(gate.required);
    assert!(!gate.passed());
}

#[test]
fn the_job_line_names_the_exit_code_on_a_failure() {
    let line = run(GateVerdict::Failed, 101).line();
    assert!(line.contains("test"), "{line}");
    assert!(line.contains("101"), "{line}");
    assert!(line.contains("2.0s"), "{line}");
}

#[test]
fn the_job_line_says_timed_out_rather_than_failed() {
    let line = run(GateVerdict::TimedOut, -1).line();
    assert!(line.contains("timed out"), "{line}");
    assert!(!line.contains("failed"), "{line}");
}

#[test]
fn the_job_line_says_cancelled_rather_than_failed() {
    let line = run(GateVerdict::Cancelled, -1).line();
    assert!(line.contains("cancelled"), "{line}");
    assert!(!line.contains("failed"), "{line}");
}

#[test]
fn the_execution_names_the_actual_verdict() {
    use magistr_engine::native::CheckExecution;
    assert_eq!(
        run(GateVerdict::Passed, 0).execution(),
        CheckExecution::Exited { code: 0 }
    );
    assert_eq!(
        run(GateVerdict::Failed, 101).execution(),
        CheckExecution::Exited { code: 101 }
    );
    assert_eq!(
        run(GateVerdict::TimedOut, -1).execution(),
        CheckExecution::TimedOut
    );
    assert_eq!(
        run(GateVerdict::Cancelled, -1).execution(),
        CheckExecution::Cancelled
    );
    assert!(!CheckExecution::TimedOut.passed() && CheckExecution::TimedOut.verified());
}
