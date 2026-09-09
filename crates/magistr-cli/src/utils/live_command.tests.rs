use std::path::PathBuf;
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use super::*;

fn dir() -> PathBuf {
    std::env::current_dir().expect("a working directory")
}

fn never() -> CancellationToken {
    CancellationToken::new()
}

#[cfg(unix)]
#[tokio::test]
async fn raw_bytes_reach_the_sink_and_the_transcript_is_stripped() {
    let mut seen: Vec<u8> = Vec::new();
    let result = run_command_live(
        "printf '\\033[31mred\\033[0m\\n'",
        &dir(),
        10,
        &never(),
        |bytes| seen.extend_from_slice(bytes),
    )
    .await;
    assert_eq!(result.exit_code, 0);
    assert!(!result.timed_out && !result.cancelled);
    // The pane gets the programme's own colour, escapes and all.
    assert!(
        seen.windows(5).any(|w| w == b"\x1b[31m"),
        "the sink saw no escape: {seen:?}"
    );
    // The record gets plain text, the pty's CRLF folded to one break.
    assert_eq!(result.stdout, "red\n");
    assert_eq!(result.stderr, "");
}

#[cfg(unix)]
#[tokio::test]
async fn the_child_sees_a_terminal_on_both_streams() {
    let result = run_command_live("test -t 1 && test -t 2", &dir(), 10, &never(), |_| {}).await;
    assert_eq!(result.exit_code, 0, "{result:?}");
}

#[cfg(unix)]
#[tokio::test]
async fn both_streams_arrive_as_one_transcript_in_order() {
    let result = run_command_live(
        "echo out; echo err 1>&2; echo out-again",
        &dir(),
        10,
        &never(),
        |_| {},
    )
    .await;
    assert_eq!(result.stdout, "out\nerr\nout-again\n");
}

#[cfg(unix)]
#[tokio::test]
async fn a_multiline_transcript_keeps_the_lines_a_pipe_would_have() {
    let result = run_command_live("printf 'a\\nb\\n'", &dir(), 10, &never(), |_| {}).await;
    assert_eq!(result.stdout, "a\nb\n");
}

#[cfg(unix)]
#[tokio::test]
async fn the_exit_code_is_the_childs() {
    let result = run_command_live("exit 42", &dir(), 10, &never(), |_| {}).await;
    assert_eq!(result.exit_code, 42);
}

#[cfg(unix)]
#[tokio::test]
async fn output_written_just_before_the_exit_is_not_lost() {
    // Many small writes then an immediate exit: the drain after the
    // child's exit must read what the kernel still holds.
    let cmd = "i=0; while [ $i -lt 200 ]; do echo line-$i; i=$((i+1)); done";
    let result = run_command_live(cmd, &dir(), 10, &never(), |_| {}).await;
    assert_eq!(result.stdout.lines().count(), 200, "{}", result.stdout);
    assert!(result.stdout.ends_with("line-199\n"));
}

#[cfg(unix)]
#[tokio::test]
async fn a_timeout_ends_the_process_group() {
    let started = std::time::Instant::now();
    let result = run_command_live("sleep 60", &dir(), 1, &never(), |_| {}).await;
    assert!(result.timed_out);
    assert_eq!(result.exit_code, -1);
    assert!(result.stderr.contains("timed out"), "{result:?}");
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "{:?}",
        started.elapsed()
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_cancel_ends_it_and_says_so() {
    let cancel = CancellationToken::new();
    let trigger = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        trigger.cancel();
    });
    let result = run_command_live("sleep 60", &dir(), 30, &cancel, |_| {}).await;
    assert!(result.cancelled);
    assert!(!result.timed_out);
    assert_eq!(result.exit_code, -1);
}

#[cfg(unix)]
#[tokio::test]
async fn a_command_that_reads_stdin_gets_eof() {
    let result = run_command_live("cat", &dir(), 5, &never(), |_| {}).await;
    assert_eq!(result.exit_code, 0);
    assert!(!result.timed_out);
}

#[tokio::test]
async fn an_empty_command_is_skipped_as_the_pipe_runner_skips_it() {
    let mut seen: Vec<u8> = Vec::new();
    let result = run_command_live("", &dir(), 10, &never(), |bytes| {
        seen.extend_from_slice(bytes)
    })
    .await;
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout, "skipped (no command configured)");
    // The text-only fallback still tells the pane what happened.
    assert_eq!(seen, b"skipped (no command configured)");
}
