use super::*;
use std::time::Duration;

fn dir() -> std::path::PathBuf {
    std::env::current_dir().expect("a working directory")
}

fn never() -> CancellationToken {
    CancellationToken::new()
}

/// The sink a test that does not watch the stream passes.
fn quiet(_: &[u8]) {}

#[tokio::test]
async fn unconfigured_gate_is_never_passing_evidence() {
    for command in ["", "   "] {
        let gate = run_gate("test", command, true, &dir(), 10, &never(), quiet).await;
        assert_eq!(gate.verdict, GateVerdict::Unconfigured);
        assert!(!gate.passed());
        assert!(!gate.verdict.is_evidence());
        assert!(gate.output.contains("no check ran"));
        assert!(matches!(
            gate.execution(),
            magistr_engine::native::CheckExecution::LaunchFailed { .. }
        ));
    }
}

#[tokio::test]
async fn a_clean_exit_passes_and_keeps_its_command() {
    let gate = run_gate("check", "true", true, &dir(), 10, &never(), quiet).await;
    assert_eq!(gate.verdict, GateVerdict::Passed);
    assert_eq!(gate.exit_code, 0);
    assert_eq!(gate.name, "check");
    assert_eq!(gate.command, "true");
    assert!(gate.required);
}

#[tokio::test]
async fn a_non_zero_exit_keeps_the_code_the_process_exited_with() {
    let gate = run_gate("lint", "exit 42", true, &dir(), 10, &never(), quiet).await;
    assert_eq!(gate.verdict, GateVerdict::Failed);
    assert_eq!(gate.exit_code, 42, "the real code, not a bool");
}

#[tokio::test]
async fn both_streams_are_kept_not_only_the_one_that_had_content() {
    let gate = run_gate(
        "test",
        "echo on-stdout; echo on-stderr 1>&2; exit 1",
        true,
        &dir(),
        10,
        &never(),
        quiet,
    )
    .await;
    assert!(gate.output.contains("on-stdout"), "{}", gate.output);
    assert!(gate.output.contains("on-stderr"), "{}", gate.output);
}

#[tokio::test]
async fn the_output_of_a_passing_gate_is_kept_too() {
    // The driver used to keep nothing at all; a green gate's output is
    // what a later "why did this pass here and fail in CI" needs.
    let gate = run_gate("check", "echo all-good", true, &dir(), 10, &never(), quiet).await;
    assert!(gate.output.contains("all-good"), "{}", gate.output);
}

#[cfg(unix)]
#[tokio::test]
async fn the_stream_carries_the_colour_and_the_record_keeps_the_text() {
    // On the pty the command paints; the sink sees every byte in order
    // while the transcript the trace keeps is the plain text, so the
    // record reads as it did off a pipe.
    let mut seen: Vec<u8> = Vec::new();
    let gate = run_gate(
        "check",
        "printf '\\033[1;32mall-good\\033[0m\\n'",
        true,
        &dir(),
        10,
        &never(),
        |bytes| seen.extend_from_slice(bytes),
    )
    .await;
    assert_eq!(gate.verdict, GateVerdict::Passed);
    assert!(
        seen.windows(7).any(|w| w == b"\x1b[1;32m"),
        "the escape reached the sink: {seen:?}"
    );
    assert_eq!(gate.output, "all-good\n");
    assert!(gate.queue_duration <= gate.duration);
    assert_eq!(gate.details(), gate.output);
}

#[tokio::test]
async fn failure_rows_are_parsed_from_the_output() {
    let gate = run_gate(
        "check",
        "echo 'error[E0308]: mismatched types'; echo '  --> src/main.rs:3:9'; exit 1",
        true,
        &dir(),
        10,
        &never(),
        quiet,
    )
    .await;
    assert!(!gate.rows.is_empty(), "rows: {:?}", gate.rows);
    let row = &gate.rows[0];
    assert_eq!(row.code.as_deref(), Some("E0308"));
    assert_eq!(row.file.as_deref(), Some("src/main.rs"));
    assert_eq!(row.line, Some(3));
}

#[tokio::test]
async fn a_timeout_is_its_own_verdict_and_kills_the_child() {
    let gate = run_gate("test", "sleep 60", true, &dir(), 1, &never(), quiet).await;
    assert_eq!(gate.verdict, GateVerdict::TimedOut);
    assert!(!gate.passed());
    assert!(
        gate.duration < Duration::from_secs(30),
        "{:?}",
        gate.duration
    );
}

#[tokio::test]
async fn a_cancel_is_its_own_verdict() {
    let cancel = CancellationToken::new();
    let token = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(150)).await;
        token.cancel();
    });
    let gate = run_gate("test", "sleep 60", true, &dir(), 30, &cancel, quiet).await;
    assert_eq!(gate.verdict, GateVerdict::Cancelled);
    assert!(!gate.passed());
}

#[tokio::test]
async fn oversized_output_is_truncated_at_the_shared_budget() {
    let cmd = format!("printf 'x%.0s' $(seq 1 {})", MAX_OUTPUT_BYTES + 10_000);
    let gate = run_gate("check", &cmd, true, &dir(), 60, &never(), quiet).await;
    assert!(
        gate.output.len() <= MAX_OUTPUT_BYTES + 200,
        "{}",
        gate.output.len()
    );
    assert!(
        gate.output.contains("[... truncated,"),
        "the marker survives"
    );
}

#[tokio::test]
async fn rows_are_parsed_from_the_whole_output_before_it_is_truncated() {
    // The diagnostic that matters can sit past the truncation point.
    let filler = format!("printf 'x%.0s' $(seq 1 {})", MAX_OUTPUT_BYTES);
    let cmd = format!(
        "{filler}; echo; echo 'error[E0433]: failed to resolve'; echo '  --> src/late.rs:9:1'; exit 1"
    );
    let gate = run_gate("check", &cmd, true, &dir(), 60, &never(), quiet).await;
    assert!(
        gate.rows.iter().any(|r| r.code.as_deref() == Some("E0433")),
        "a row past the truncation point survives: {:?}",
        gate.rows
    );
}

#[tokio::test]
async fn a_gate_reads_nothing_from_stdin() {
    // A command that reads stdin must get EOF, never block the loop.
    let gate = run_gate("check", "cat", true, &dir(), 5, &never(), quiet).await;
    assert_eq!(gate.verdict, GateVerdict::Passed);
}

#[tokio::test]
async fn a_not_required_gate_says_so() {
    let gate = run_gate("mock-parity", "exit 1", false, &dir(), 10, &never(), quiet).await;
    assert!(!gate.required);
    assert_eq!(gate.verdict, GateVerdict::Failed);
}
