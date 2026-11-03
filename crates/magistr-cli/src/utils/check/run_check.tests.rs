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
async fn unconfigured_check_is_never_passing_evidence() {
    for command in ["", "   "] {
        let check = run_check("test", command, true, &dir(), 10, &never(), quiet).await;
        assert_eq!(check.verdict, CheckVerdict::Unconfigured);
        assert!(!check.passed());
        assert!(!check.verdict.is_evidence());
        assert!(check.output.contains("no check ran"));
        assert!(matches!(
            check.execution(),
            magistr_engine::native::CheckExecution::LaunchFailed { .. }
        ));
    }
}

#[tokio::test]
async fn a_clean_exit_passes_and_keeps_its_command() {
    let check = run_check("check", "true", true, &dir(), 10, &never(), quiet).await;
    assert_eq!(check.verdict, CheckVerdict::Passed);
    assert_eq!(check.exit_code, 0);
    assert_eq!(check.name, "check");
    assert_eq!(check.command, "true");
    assert!(check.required);
}

#[tokio::test]
async fn a_non_zero_exit_keeps_the_code_the_process_exited_with() {
    let check = run_check("lint", "exit 42", true, &dir(), 10, &never(), quiet).await;
    assert_eq!(check.verdict, CheckVerdict::Failed);
    assert_eq!(check.exit_code, 42, "the real code, not a bool");
}

#[tokio::test]
async fn both_streams_are_kept_not_only_the_one_that_had_content() {
    let check = run_check(
        "test",
        "echo on-stdout; echo on-stderr 1>&2; exit 1",
        true,
        &dir(),
        10,
        &never(),
        quiet,
    )
    .await;
    assert!(check.output.contains("on-stdout"), "{}", check.output);
    assert!(check.output.contains("on-stderr"), "{}", check.output);
}

#[tokio::test]
async fn the_output_of_a_passing_check_is_kept_too() {
    // The driver used to keep nothing at all; a green check's output is
    // what a later "why did this pass here and fail in CI" needs.
    let check = run_check("check", "echo all-good", true, &dir(), 10, &never(), quiet).await;
    assert!(check.output.contains("all-good"), "{}", check.output);
}

#[cfg(unix)]
#[tokio::test]
async fn the_stream_carries_the_colour_and_the_record_keeps_the_text() {
    // On the pty the command paints; the sink sees every byte in order
    // while the transcript the trace keeps is the plain text, so the
    // record reads as it did off a pipe.
    let mut seen: Vec<u8> = Vec::new();
    let check = run_check(
        "check",
        "printf '\\033[1;32mall-good\\033[0m\\n'",
        true,
        &dir(),
        10,
        &never(),
        |bytes| seen.extend_from_slice(bytes),
    )
    .await;
    assert_eq!(check.verdict, CheckVerdict::Passed);
    assert!(
        seen.windows(7).any(|w| w == b"\x1b[1;32m"),
        "the escape reached the sink: {seen:?}"
    );
    assert_eq!(check.output, "all-good\n");
    assert!(check.queue_duration <= check.duration);
    assert_eq!(check.details(), check.output);
}

#[tokio::test]
async fn failure_rows_are_parsed_from_the_output() {
    let check = run_check(
        "check",
        "echo 'error[E0308]: mismatched types'; echo '  --> src/main.rs:3:9'; exit 1",
        true,
        &dir(),
        10,
        &never(),
        quiet,
    )
    .await;
    assert!(!check.rows.is_empty(), "rows: {:?}", check.rows);
    let row = &check.rows[0];
    assert_eq!(row.code.as_deref(), Some("E0308"));
    assert_eq!(row.file.as_deref(), Some("src/main.rs"));
    assert_eq!(row.line, Some(3));
}

#[tokio::test]
async fn a_timeout_is_its_own_verdict_and_kills_the_child() {
    let check = run_check("test", "sleep 60", true, &dir(), 1, &never(), quiet).await;
    assert_eq!(check.verdict, CheckVerdict::TimedOut);
    assert!(!check.passed());
    assert!(
        check.duration < Duration::from_secs(30),
        "{:?}",
        check.duration
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
    let check = run_check("test", "sleep 60", true, &dir(), 30, &cancel, quiet).await;
    assert_eq!(check.verdict, CheckVerdict::Cancelled);
    assert!(!check.passed());
}

#[tokio::test]
async fn oversized_output_is_truncated_at_the_shared_budget() {
    let cmd = format!("printf 'x%.0s' $(seq 1 {})", MAX_OUTPUT_BYTES + 10_000);
    let check = run_check("check", &cmd, true, &dir(), 60, &never(), quiet).await;
    assert!(
        check.output.len() <= MAX_OUTPUT_BYTES + 200,
        "{}",
        check.output.len()
    );
    assert!(
        check.output.contains("[... truncated,"),
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
    let check = run_check("check", &cmd, true, &dir(), 60, &never(), quiet).await;
    assert!(
        check
            .rows
            .iter()
            .any(|r| r.code.as_deref() == Some("E0433")),
        "a row past the truncation point survives: {:?}",
        check.rows
    );
}

#[tokio::test]
async fn a_check_reads_nothing_from_stdin() {
    // A command that reads stdin must get EOF, never block the loop.
    let check = run_check("check", "cat", true, &dir(), 5, &never(), quiet).await;
    assert_eq!(check.verdict, CheckVerdict::Passed);
}

#[tokio::test]
async fn a_not_required_check_says_so() {
    let check = run_check("mock-parity", "exit 1", false, &dir(), 10, &never(), quiet).await;
    assert!(!check.required);
    assert_eq!(check.verdict, CheckVerdict::Failed);
}

#[tokio::test]
async fn configured_deadlines_apply_and_invalid_policy_cannot_start_a_command() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join(".magistr")).unwrap();
    let mut config = magistr_core::validate::load_project_config(&dir().join("../..")).unwrap();
    config.verification.timeout_secs = Some(1);
    config.verification.checks.clear();
    let path = root.path().join(".magistr/project.json");
    std::fs::write(&path, serde_json::to_vec(&config).unwrap()).unwrap();
    let timed = run_check("bounded", "sleep 30", true, root.path(), 0, &never(), quiet).await;
    assert_eq!(timed.verdict, CheckVerdict::TimedOut);
    assert!(timed.duration < Duration::from_secs(4));
    config.verification.max_parallel = 0;
    std::fs::write(&path, serde_json::to_vec(&config).unwrap()).unwrap();
    let refused = run_check(
        "invalid",
        "touch should-not-run",
        true,
        root.path(),
        0,
        &never(),
        quiet,
    )
    .await;
    assert_eq!(refused.verdict, CheckVerdict::LaunchFailed);
    assert!(matches!(
        refused.execution(),
        magistr_engine::native::CheckExecution::LaunchFailed { .. }
    ));
    assert!(!root.path().join("should-not-run").exists());
}
