use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use super::*;

#[tokio::test]
async fn successful_command_returns_output() {
    let result = run_command_with_timeout("echo hello", Path::new("."), 30, None).await;
    assert!(!result.timed_out);
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout.trim(), "hello");
    assert!(result.stderr.is_empty());
}

#[tokio::test]
async fn failed_command_returns_nonzero_exit() {
    let result = run_command_with_timeout("exit 42", Path::new("."), 30, None).await;
    assert!(!result.timed_out);
    assert_eq!(result.exit_code, 42);
}

#[tokio::test]
async fn stderr_is_captured() {
    let result = run_command_with_timeout("echo oops >&2", Path::new("."), 30, None).await;
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stderr.trim(), "oops");
}

#[tokio::test]
async fn timeout_kills_long_running_command() {
    let result = run_command_with_timeout("sleep 60", Path::new("."), 1, None).await;
    assert!(result.timed_out);
    assert_ne!(result.exit_code, 0);
}

#[tokio::test]
async fn empty_command_returns_skip() {
    let result = run_command_with_timeout("", Path::new("."), 30, None).await;
    assert!(!result.timed_out);
    assert_eq!(result.exit_code, 0);
    assert!(result.stdout.contains("skipped"));
}

#[tokio::test]
async fn progress_callback_is_invoked_for_long_command() {
    let count = Arc::new(AtomicU32::new(0));
    let count_clone = count.clone();
    let callback: ProgressCallback = Box::new(move |_elapsed| {
        count_clone.fetch_add(1, Ordering::Relaxed);
    });

    // Command that runs ~3s with 1s heartbeat interval
    let result = run_command_with_timeout_interval(
        "sleep 3",
        Path::new("."),
        10,
        Some(callback),
        Duration::from_secs(1),
    )
    .await;

    assert!(!result.timed_out);
    assert_eq!(result.exit_code, 0);
    // Should have been called at least twice (at 1s and 2s)
    assert!(count.load(Ordering::Relaxed) >= 2);
}

#[tokio::test]
async fn child_processes_are_reaped_after_timeout() {
    // Spawn a command that creates a child process tree, then timeout
    // The parent sleeps, so timeout triggers graceful kill
    let result = run_command_with_timeout("sh -c 'sleep 60'", Path::new("."), 1, None).await;
    assert!(result.timed_out);
    // After return, all children should be cleaned up (no zombies)
    // We can't easily assert this, but the test verifies no panic/hang
}

#[cfg(unix)]
#[tokio::test]
async fn cancel_kills_the_command_and_its_process_group() {
    let tmp = tempfile::TempDir::new().unwrap();
    let pid_path = tmp.path().join("run.pid");
    let cmd = format!("echo $$ > '{}' && sleep 60", pid_path.display());
    let cancel = CancellationToken::new();
    let signal = cancel.clone();
    let task = tokio::spawn(async move {
        run_command_cancellable(&cmd, Path::new("."), 60, None, &cancel).await
    });

    // Wait for the shell to be up: it writes its own pid (the process
    // group leader under setsid) before sleeping.
    let deadline = Instant::now() + Duration::from_secs(5);
    while !pid_path.exists() && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let pid: i32 = std::fs::read_to_string(&pid_path)
        .expect("the shell wrote its pid")
        .trim()
        .parse()
        .expect("a pid");

    signal.cancel();
    let result = tokio::time::timeout(Duration::from_secs(10), task)
        .await
        .expect("cancellation ends the run within the grace")
        .expect("the run did not panic");
    assert!(result.cancelled);
    assert!(!result.timed_out);
    assert_ne!(result.exit_code, 0);

    // The group leader is gone — a sleeping sh dies on the SIGTERM.
    let deadline = Instant::now() + Duration::from_secs(5);
    // SAFETY: signal 0 only probes for existence.
    while unsafe { libc::kill(pid, 0) } == 0 {
        assert!(
            Instant::now() < deadline,
            "the cancelled command's process is still alive"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[cfg(unix)]
#[tokio::test]
async fn graceful_sigterm_allows_cleanup() {
    // Script that traps SIGTERM and writes a marker before exiting
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let marker_path = tmp.path().to_str().unwrap().to_string();
    // Use background sleep + wait so the shell is at a builtin `wait`
    // when SIGTERM arrives — this ensures the trap fires reliably.
    let cmd = format!("trap 'echo terminated > {marker_path}; exit 0' TERM; sleep 60 & wait");
    let result = run_command_with_timeout(&cmd, Path::new("."), 1, None).await;
    assert!(result.timed_out);

    // Give a moment for the file write to flush
    tokio::time::sleep(Duration::from_millis(200)).await;

    // The process should have received SIGTERM and written the marker
    let content = std::fs::read_to_string(&marker_path).unwrap_or_default();
    assert_eq!(content.trim(), "terminated");
}
