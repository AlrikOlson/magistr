use super::*;
use std::time::Duration;

fn argv(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| (*part).to_owned()).collect()
}

fn limits(millis: u64) -> RunLimits {
    RunLimits::new(Duration::from_millis(millis))
}

async fn run(parts: &[&str], limits: RunLimits) -> Result<RunExecution, WorkspaceError> {
    let dir = tempfile::tempdir().expect("workspace");
    ProcessExecutor::new(dir.path())
        .execute(argv(parts), limits, CancellationToken::new())
        .await
}

#[cfg(unix)]
fn group_is_gone(pid: u32) -> bool {
    // SAFETY: signal 0 probes existence without delivering anything; the pid
    // came from our own spawn and its group is what we check.
    let result = unsafe { libc::kill(-(pid as i32), 0) };
    result == -1 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
}

#[cfg(unix)]
async fn wait_for_group_exit(pid: u32) -> bool {
    for _ in 0..50 {
        if group_is_gone(pid) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    group_is_gone(pid)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_finished_command_answers_its_exit_code_and_both_streams() {
    let ok = run(
        &["sh", "-c", "echo out; echo err 1>&2; exit 0"],
        limits(5_000),
    )
    .await
    .expect("spawned");
    assert!(ok.is_complete());
    assert_eq!(ok.exit_code, Some(0));
    assert_eq!(ok.stdout, "out\n");
    assert_eq!(ok.stderr, "err\n");
    assert!(ok.pid.is_some());
    assert_eq!(ok.dropped_bytes, 0);

    let failed = run(&["sh", "-c", "exit 3"], limits(5_000))
        .await
        .expect("spawned");
    assert_eq!(failed.exit_code, Some(3));
    assert!(failed.is_complete());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_missing_program_is_a_typed_not_found_before_any_execution() {
    let error = run(&["magistr-no-such-program-9f3a"], limits(5_000))
        .await
        .expect_err("spawn fails");
    assert_eq!(error.kind, ErrorKind::FileNotFound);
    assert!(error.message.contains("magistr-no-such-program-9f3a"));
}

#[tokio::test(flavor = "multi_thread")]
async fn empty_argv_is_refused() {
    let error = run(&[], limits(5_000)).await.expect_err("nothing to spawn");
    assert_eq!(error.kind, ErrorKind::RunRedirected);
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn a_hang_is_killed_at_the_deadline_with_partial_output_as_data() {
    let started = std::time::Instant::now();
    let execution = run(&["sh", "-c", "echo partial; sleep 30"], limits(300))
        .await
        .expect("spawned");
    assert!(execution.timed_out);
    assert!(!execution.cancelled);
    assert_eq!(execution.exit_code, None);
    assert_eq!(execution.stdout, "partial\n");
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "killed promptly"
    );
    assert!(wait_for_group_exit(execution.pid.expect("pid")).await);
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn a_cancel_kills_the_whole_group_including_grandchildren() {
    let dir = tempfile::tempdir().expect("workspace");
    let executor = ProcessExecutor::new(dir.path());
    let cancel = CancellationToken::new();
    let trigger = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        trigger.cancel();
    });
    let execution = executor
        .execute(
            argv(&["sh", "-c", "sleep 30 & sleep 30 & wait"]),
            limits(30_000),
            cancel,
        )
        .await
        .expect("spawned");
    assert!(execution.cancelled);
    assert!(!execution.timed_out);
    assert_eq!(execution.exit_code, None);
    assert!(wait_for_group_exit(execution.pid.expect("pid")).await);
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn a_grandchild_holding_the_pipes_cannot_hang_the_answer() {
    // The parent exits at once; its background child keeps stdout open.
    let started = std::time::Instant::now();
    let execution = run(&["sh", "-c", "sleep 30 & echo done"], limits(400))
        .await
        .expect("spawned");
    assert_eq!(execution.exit_code, Some(0));
    assert_eq!(execution.stdout, "done\n");
    assert!(
        execution.timed_out,
        "the drain hit the deadline and killed the group"
    );
    assert!(started.elapsed() < Duration::from_secs(10));
    assert!(wait_for_group_exit(execution.pid.expect("pid")).await);
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn noisy_output_is_bounded_and_the_gap_is_counted() {
    let execution = run(
        &["seq", "1", "200000"],
        limits(20_000).with_capture_bytes(64 * 1024),
    )
    .await
    .expect("spawned");
    assert_eq!(execution.exit_code, Some(0));
    assert!(execution.dropped_bytes > 0);
    assert!(execution.stdout.starts_with("1\n2\n3\n"));
    assert!(execution.stdout.trim_end().ends_with("200000"));
    assert!(execution.stdout.len() <= 64 * 1024 + 128);
    assert!(execution.stdout.contains("bytes of output dropped"));
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn a_signalled_child_reports_the_signal_not_a_code() {
    let execution = run(&["sh", "-c", "kill -TERM $$"], limits(5_000))
        .await
        .expect("spawned");
    assert!(!execution.is_complete());
    assert_eq!(execution.exit_code, None);
    assert_eq!(execution.signal, Some(libc::SIGTERM));
    assert!(!execution.timed_out);
}
