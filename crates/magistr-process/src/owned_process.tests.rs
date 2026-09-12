use std::time::Duration;

use tokio_util::sync::CancellationToken;

use super::*;

fn shell(line: &str, millis: u64) -> ProcessSpec {
    ProcessSpec::new(Launch::Shell(line.into()), Duration::from_millis(millis))
}

fn argv(parts: &[&str], millis: u64) -> ProcessSpec {
    ProcessSpec::new(
        Launch::Argv(parts.iter().map(|part| (*part).to_owned()).collect()),
        Duration::from_millis(millis),
    )
}

async fn run(spec: ProcessSpec) -> ProcessOutcome {
    OwnedProcess::spawn(spec)
        .expect("spawned")
        .run(&CancellationToken::new(), |_| {})
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
async fn a_shell_line_and_an_argv_both_answer_their_exit_code_and_streams() {
    let ok = run(shell("echo out; echo err 1>&2; exit 0", 5_000)).await;
    assert!(ok.is_complete(), "{ok:?}");
    assert_eq!(ok.exit_code, Some(0));
    assert_eq!(ok.stdout, "out\n");
    assert_eq!(ok.stderr, "err\n");
    assert!(ok.pid.is_some());
    assert_eq!(ok.dropped_bytes, 0);

    let failed = run(argv(&["sh", "-c", "exit 3"], 5_000)).await;
    assert_eq!(failed.exit_code, Some(3));
    assert!(failed.is_complete());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_missing_program_and_an_empty_argv_fail_before_anything_runs() {
    let missing = OwnedProcess::spawn(argv(&["magistr-no-such-program-9f3a"], 5_000))
        .err()
        .expect("spawn fails");
    assert_eq!(missing.kind(), std::io::ErrorKind::NotFound);
    let empty = OwnedProcess::spawn(argv(&[], 5_000))
        .err()
        .expect("nothing to spawn");
    assert_eq!(empty.kind(), std::io::ErrorKind::InvalidInput);
}

#[tokio::test(flavor = "multi_thread")]
async fn chunks_reach_the_sink_as_they_land() {
    let mut seen = Vec::new();
    OwnedProcess::spawn(shell("printf 'a'; printf 'b' 1>&2", 5_000))
        .expect("spawned")
        .run(&CancellationToken::new(), |bytes| {
            seen.extend_from_slice(bytes);
        })
        .await;
    seen.sort_unstable();
    assert_eq!(seen, b"ab");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_working_directory_is_the_specs() {
    let dir = tempfile::tempdir().expect("dir");
    // Git's sh prints POSIX spellings; -W asks it for the Windows one.
    let pwd = if cfg!(windows) { "pwd -W" } else { "pwd" };
    let outcome = run(shell(pwd, 5_000).in_dir(dir.path())).await;
    let expected = std::fs::canonicalize(dir.path()).expect("canonical");
    assert_eq!(
        std::fs::canonicalize(outcome.stdout.trim()).expect("canonical"),
        expected
    );
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn a_hang_is_killed_at_the_deadline_with_partial_output_as_data() {
    let started = std::time::Instant::now();
    let outcome =
        run(shell("echo partial; sleep 30", 300).with_termination(Termination::Immediate)).await;
    assert!(outcome.timed_out);
    assert!(!outcome.cancelled);
    assert_eq!(outcome.exit_code, None);
    assert_eq!(outcome.stdout, "partial\n");
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "killed promptly"
    );
    assert!(wait_for_group_exit(outcome.pid.expect("pid")).await);
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn a_cancel_kills_the_whole_group_including_grandchildren() {
    let cancel = CancellationToken::new();
    let trigger = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        trigger.cancel();
    });
    let outcome = OwnedProcess::spawn(
        shell("sleep 30 & sleep 30 & wait", 30_000).with_termination(Termination::Immediate),
    )
    .expect("spawned")
    .run(&cancel, |_| {})
    .await;
    assert!(outcome.cancelled);
    assert!(!outcome.timed_out);
    assert_eq!(outcome.exit_code, None);
    assert!(wait_for_group_exit(outcome.pid.expect("pid")).await);
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn a_grandchild_holding_the_pipes_cannot_hang_the_answer() {
    // The parent exits at once; its background child keeps stdout open.
    let started = std::time::Instant::now();
    let outcome =
        run(shell("sleep 30 & echo done", 400).with_termination(Termination::Immediate)).await;
    assert_eq!(outcome.exit_code, Some(0));
    assert_eq!(outcome.stdout, "done\n");
    assert!(
        outcome.timed_out,
        "the drain hit the deadline and killed the group"
    );
    assert!(started.elapsed() < Duration::from_secs(10));
    assert!(wait_for_group_exit(outcome.pid.expect("pid")).await);
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn noisy_output_is_bounded_and_the_gap_is_counted() {
    let outcome = run(argv(&["seq", "1", "200000"], 20_000).with_capture_bytes(64 * 1024)).await;
    assert_eq!(outcome.exit_code, Some(0));
    assert!(outcome.dropped_bytes > 0);
    assert!(outcome.stdout.starts_with("1\n2\n3\n"));
    assert!(outcome.stdout.trim_end().ends_with("200000"));
    assert!(outcome.stdout.len() <= 64 * 1024 + 128);
    assert!(outcome.stdout.contains("bytes of output dropped"));
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn a_signalled_child_reports_the_signal_not_a_code() {
    let outcome = run(shell("kill -TERM $$", 5_000)).await;
    assert!(!outcome.is_complete());
    assert_eq!(outcome.exit_code, None);
    assert_eq!(outcome.signal, Some(libc::SIGTERM));
    assert!(!outcome.timed_out);
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn a_graceful_termination_lets_a_trap_run_before_the_kill() {
    let tmp = tempfile::NamedTempFile::new().expect("marker");
    let marker = tmp.path().display().to_string();
    // Background sleep + wait so the shell is at a builtin `wait` when
    // SIGTERM arrives — the trap fires reliably.
    let line = format!("trap 'echo terminated > {marker}; exit 0' TERM; sleep 60 & wait");
    let outcome = run(shell(&line, 300).with_termination(Termination::Graceful {
        grace: Duration::from_secs(5),
    }))
    .await;
    assert!(outcome.timed_out);
    tokio::time::sleep(Duration::from_millis(200)).await;
    let content = std::fs::read_to_string(tmp.path()).unwrap_or_default();
    assert_eq!(content.trim(), "terminated");
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn on_a_terminal_the_child_sees_a_tty_and_both_streams_are_one_raw_transcript() {
    let process = OwnedProcess::spawn(
        shell(
            "test -t 1 && test -t 2 && printf '\\033[31mred\\033[0m\\n'; echo err 1>&2",
            5_000,
        )
        .on_terminal(120, 40),
    )
    .expect("spawned");
    assert!(process.is_terminal());
    let mut seen = Vec::new();
    let outcome = process
        .run(&CancellationToken::new(), |bytes| {
            seen.extend_from_slice(bytes);
        })
        .await;
    assert_eq!(outcome.exit_code, Some(0), "{outcome:?}");
    assert!(
        seen.windows(5).any(|w| w == b"\x1b[31m"),
        "raw escapes reach the sink"
    );
    assert_eq!(outcome.stdout, "\x1b[31mred\x1b[0m\r\nerr\r\n");
    assert_eq!(outcome.stderr, "");
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn on_a_terminal_output_written_just_before_the_exit_is_not_lost() {
    let line = "i=0; while [ $i -lt 200 ]; do echo line-$i; i=$((i+1)); done";
    let outcome = run(shell(line, 10_000).on_terminal(120, 40)).await;
    assert_eq!(outcome.stdout.lines().count(), 200, "{}", outcome.stdout);
    assert!(outcome.stdout.ends_with("line-199\r\n"));
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn on_a_terminal_a_timeout_and_a_cancel_end_the_group() {
    let timed = run(shell("sleep 60", 300).on_terminal(80, 24)).await;
    assert!(timed.timed_out && !timed.cancelled);
    assert_eq!(timed.exit_code, None);
    assert!(wait_for_group_exit(timed.pid.expect("pid")).await);

    let cancel = CancellationToken::new();
    let trigger = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        trigger.cancel();
    });
    let cancelled = OwnedProcess::spawn(shell("sleep 60", 30_000).on_terminal(80, 24))
        .expect("spawned")
        .run(&cancel, |_| {})
        .await;
    assert!(cancelled.cancelled && !cancelled.timed_out);
    assert!(wait_for_group_exit(cancelled.pid.expect("pid")).await);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_command_that_reads_stdin_gets_eof() {
    let outcome = run(shell("cat", 5_000)).await;
    assert_eq!(outcome.exit_code, Some(0));
    assert!(!outcome.timed_out);
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn dropping_a_running_process_kills_its_child() {
    let dir = tempfile::tempdir().expect("dir");
    let started = dir.path().join("started");
    let survived = dir.path().join("survived");
    let line = format!(
        "touch {} && sleep 2 && touch {}",
        started.display(),
        survived.display()
    );
    let process = OwnedProcess::spawn(shell(&line, 30_000)).expect("spawned");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while !started.exists() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "the child never started"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    drop(process);
    tokio::time::sleep(Duration::from_millis(2_500)).await;
    assert!(!survived.exists(), "the child outlived the dropped process");
}
