use super::*;
use std::time::Duration;

fn run(verdict: GateVerdict, exit_code: i32) -> GateRun {
    GateRun {
        name: "test".to_owned(),
        command: "cargo nextest run".to_owned(),
        required: true,
        verdict,
        exit_code,
        first_exit_code: None,
        output: String::new(),
        rows: Vec::new(),
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
fn a_flake_keeps_both_exit_codes() {
    let first = run(GateVerdict::Failed, 101);
    let second = run(GateVerdict::Passed, 0);
    let flaky = second.as_flake_of(&first);
    assert_eq!(flaky.verdict, GateVerdict::Flaky);
    assert_eq!(flaky.exit_code, 0, "the retry's code is the run's code");
    assert_eq!(
        flaky.first_exit_code,
        Some(101),
        "the failing attempt's code is kept, or the flake is unprovable"
    );
    assert!(flaky.passed(), "green on the retry does not stop the loop");
    let line = flaky.line();
    assert!(line.contains("flaky"), "{line}");
    assert!(
        line.contains("101"),
        "the line names what it failed with: {line}"
    );
}

#[test]
fn a_flake_keeps_the_failing_attempts_output_not_the_green_ones() {
    // The green retry has nothing to say; the red attempt is the evidence.
    let mut first = run(GateVerdict::Failed, 101);
    first.output = "thread 'a::b' panicked".to_owned();
    first.rows = vec![magistr_core::GateFailureRow {
        severity: magistr_core::GateRowSeverity::TestFailure,
        code: None,
        message: "a::b".to_owned(),
        file: None,
        line: None,
        column: None,
    }];
    let flaky = run(GateVerdict::Passed, 0).as_flake_of(&first);
    assert!(flaky.output.contains("panicked"), "{}", flaky.output);
    assert_eq!(flaky.rows.len(), 1);
}

#[test]
fn the_durations_of_both_attempts_are_added() {
    let first = run(GateVerdict::Failed, 101);
    let flaky = run(GateVerdict::Passed, 0).as_flake_of(&first);
    assert_eq!(flaky.duration, Duration::from_secs(4));
}

#[test]
fn a_run_that_should_be_retried_is_a_red_required_one_and_nothing_else() {
    assert!(run(GateVerdict::Failed, 101).is_retryable());
    // A timeout re-run costs another whole timeout for the same answer.
    assert!(!run(GateVerdict::TimedOut, -1).is_retryable());
    // A cancel means the cockpit is closing; there is nothing to retry into.
    assert!(!run(GateVerdict::Cancelled, -1).is_retryable());
    assert!(!run(GateVerdict::Passed, 0).is_retryable());
    // A gate registered not-required is expected to fail; paying twice for
    // the same expected red is the mock-parity tax this rule avoids.
    let mut optional = run(GateVerdict::Failed, 1);
    optional.required = false;
    assert!(!optional.is_retryable());
    // And a flake is already the product of a retry.
    assert!(!run(GateVerdict::Flaky, 0).is_retryable());
}

#[test]
fn the_execution_names_the_verdict_and_a_flake_is_its_passing_retry() {
    use magistr_engine::tas::CheckExecution;
    assert_eq!(
        run(GateVerdict::Passed, 0).execution(),
        CheckExecution::Exited { code: 0 }
    );
    assert_eq!(
        run(GateVerdict::Failed, 101).execution(),
        CheckExecution::Exited { code: 101 }
    );
    let mut flaky = run(GateVerdict::Flaky, 0);
    flaky.first_exit_code = Some(101);
    assert_eq!(flaky.execution(), CheckExecution::Exited { code: 0 });
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
