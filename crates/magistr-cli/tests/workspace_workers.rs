//! Worker containment through real native processes and the private CLI role.

mod workspace_worker_support;

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};

use magistr_workspace::worker::{
    WorkerError, WorkerOptions, WorkerRequest, WorkerResponse, WorkerSupervisor,
};
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;
use workspace_worker_support::{
    WAIT, event, events, fixture, options, pending, read_frame, reaped, write_frame,
};

fn magistr_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_magistr"))
}

async fn ping(worker: &WorkerSupervisor) -> Result<WorkerResponse, WorkerError> {
    worker
        .call(WorkerRequest::Ping, WAIT, &CancellationToken::new())
        .await
}

#[tokio::test]
async fn private_native_role_is_ready_and_round_trips_without_mcp() {
    let worker = WorkerSupervisor::new(magistr_binary(), WorkerOptions::default())
        .expect("native supervisor");
    assert!(matches!(ping(&worker).await, Ok(WorkerResponse::Pong)));
    let status = worker.status().await;
    let pid = status.pid.expect("real worker process");
    assert_ne!(pid, std::process::id());
    assert_eq!(worker.pending(), 0);
    worker.shutdown().await.expect("native shutdown");
    reaped(&[pid]).await;
    assert!(matches!(ping(&worker).await, Err(WorkerError::Stopped)));
}

#[tokio::test]
async fn native_worker_rejects_malformed_oversize_and_wrong_version_frames() {
    for case in ["malformed", "empty", "oversize", "version", "truncated"] {
        let mut child = tokio::process::Command::new(magistr_binary())
            .arg("--workspace-worker")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .expect("private worker process");
        let pid = child.id().expect("worker pid");
        let mut input = child.stdin.take().expect("worker stdin");
        let mut output = child.stdout.take().expect("worker stdout");
        let ready = read_frame(&mut output).await;
        assert_eq!(ready["v"], 1, "{case}: versioned readiness");
        assert_eq!(ready["kind"], "ready", "{case}: explicit readiness");
        match case {
            "malformed" => write_frame(&mut input, b"invalid-json").await,
            "empty" => write_frame(&mut input, b"").await,
            "oversize" => {
                input
                    .write_u32_le(32 * 1024 * 1024 + 1)
                    .await
                    .expect("oversize length");
                input.flush().await.expect("flush oversize length");
            }
            "version" => {
                let request = serde_json::json!({
                    "v": 2, "kind": "request", "id": 1, "request": WorkerRequest::Ping,
                });
                write_frame(
                    &mut input,
                    &serde_json::to_vec(&request).expect("request JSON"),
                )
                .await;
            }
            "truncated" => {
                input.write_u32_le(128).await.expect("truncated length");
                input.write_all(b"x").await.expect("truncated body");
            }
            _ => unreachable!(),
        }
        drop(input);
        let exit = tokio::time::timeout(WAIT, child.wait())
            .await
            .unwrap_or_else(|_| panic!("{case}: invalid input must terminate the worker"))
            .expect("worker reaped");
        assert!(!exit.success(), "{case}: reject malformed input");
        reaped(&[pid]).await;
    }
}

#[tokio::test]
async fn stalled_startup_has_a_readiness_deadline_and_is_reaped() {
    let directory = tempfile::tempdir().expect("test directory");
    let log = directory.path().join("worker.log");
    let worker = fixture(
        "stall-start",
        &log,
        WorkerOptions {
            // Long enough for the stalled worker to launch and log its start,
            // still far below the call timeout this deadline must not wait for.
            ready_timeout: Duration::from_secs(1),
            ..options()
        },
    );
    let started = Instant::now();
    let error = ping(&worker).await.expect_err("no readiness");
    assert!(
        matches!(error, WorkerError::Deadline | WorkerError::Unavailable(_)),
        "{error:?}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "readiness must bound startup independently of call timeout"
    );
    let pids = event(&log, "start", 1).await;
    reaped(&pids).await;
    assert_eq!(worker.pending(), 0);
    worker
        .shutdown()
        .await
        .expect("shutdown after startup failure");
}

#[tokio::test]
async fn malformed_worker_replies_are_contained_and_reaped() {
    for mode in [
        "malformed-response",
        "oversize-response",
        "wrong-id-response",
        "wrong-version-response",
        "wrong-version-ready",
    ] {
        let directory = tempfile::tempdir().expect("test directory");
        let log = directory.path().join("worker.log");
        let worker = fixture(mode, &log, options());
        let error = ping(&worker).await.expect_err("invalid worker response");
        if mode.starts_with("wrong-version") {
            assert_eq!(error, WorkerError::UnsupportedVersion, "{mode}");
        } else {
            assert!(
                matches!(error, WorkerError::Protocol(_)),
                "{mode}: {error:?}"
            );
        }
        reaped(&event(&log, "start", 1).await).await;
        assert_eq!(worker.pending(), 0);
        worker.shutdown().await.expect("shutdown failed worker");
    }
}

#[tokio::test]
async fn oversized_outgoing_requests_are_refused_without_disturbing_a_ready_worker() {
    let worker = WorkerSupervisor::new(magistr_binary(), WorkerOptions::default())
        .expect("native supervisor");
    assert!(matches!(ping(&worker).await, Ok(WorkerResponse::Pong)));
    let pid = worker.status().await.pid.expect("ready worker pid");
    let request = WorkerRequest::Parse {
        path: "oversized.rs".into(),
        language: "rust".into(),
        source: "x".repeat(32 * 1024 * 1024),
    };
    assert!(matches!(
        worker.call(request, WAIT, &CancellationToken::new()).await,
        Err(WorkerError::Protocol(_))
    ));
    assert_eq!(worker.pending(), 0);
    assert_eq!(worker.status().await.pid, Some(pid));
    assert!(matches!(ping(&worker).await, Ok(WorkerResponse::Pong)));
    worker.shutdown().await.expect("shutdown");
    reaped(&[pid]).await;
}

#[tokio::test]
async fn queue_admission_is_bounded_and_queued_cancellation_does_not_kill_active_work() {
    let directory = tempfile::tempdir().expect("test directory");
    let log = directory.path().join("worker.log");
    let worker = Arc::new(fixture(
        "stall",
        &log,
        WorkerOptions {
            max_pending: 2,
            ..options()
        },
    ));
    let active_cancel = CancellationToken::new();
    let active = {
        let worker = Arc::clone(&worker);
        let cancel = active_cancel.clone();
        tokio::spawn(async move { worker.call(WorkerRequest::Ping, WAIT, &cancel).await })
    };
    let pid = event(&log, "request", 1).await[0];
    let queued_cancel = CancellationToken::new();
    let queued = {
        let worker = Arc::clone(&worker);
        let cancel = queued_cancel.clone();
        tokio::spawn(async move { worker.call(WorkerRequest::Ping, WAIT, &cancel).await })
    };
    pending(&worker, 2).await;
    assert!(matches!(ping(&worker).await, Err(WorkerError::Busy)));
    queued_cancel.cancel();
    assert!(matches!(
        queued.await.expect("queued task"),
        Err(WorkerError::Cancelled)
    ));
    assert_eq!(worker.pending(), 1);
    #[cfg(unix)]
    assert!(
        workspace_worker_support::is_alive(pid),
        "queued cancellation preserves active process"
    );
    assert_eq!(
        events(&log, "request").len(),
        1,
        "cancelled queued work never sent"
    );
    active_cancel.cancel();
    assert!(matches!(
        active.await.expect("active task"),
        Err(WorkerError::Cancelled)
    ));
    reaped(&[pid]).await;
    assert_eq!(worker.pending(), 0);
    worker.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn queued_deadlines_expire_without_waiting_for_the_active_request() {
    let directory = tempfile::tempdir().expect("test directory");
    let log = directory.path().join("worker.log");
    let worker = Arc::new(fixture("stall", &log, options()));
    let cancel = CancellationToken::new();
    let active = {
        let worker = Arc::clone(&worker);
        let token = cancel.clone();
        tokio::spawn(async move { worker.call(WorkerRequest::Ping, WAIT, &token).await })
    };
    let pid = event(&log, "request", 1).await[0];
    let started = Instant::now();
    assert!(matches!(
        worker
            .call(
                WorkerRequest::Ping,
                Duration::from_millis(150),
                &CancellationToken::new()
            )
            .await,
        Err(WorkerError::Deadline)
    ));
    assert!(started.elapsed() < Duration::from_secs(2));
    assert_eq!(worker.pending(), 1);
    #[cfg(unix)]
    assert!(workspace_worker_support::is_alive(pid));
    cancel.cancel();
    assert!(matches!(
        active.await.expect("active task"),
        Err(WorkerError::Cancelled)
    ));
    reaped(&[pid]).await;
    worker.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn active_deadline_reaps_worker_and_pre_cancelled_calls_never_spawn() {
    let directory = tempfile::tempdir().expect("test directory");
    let log = directory.path().join("worker.log");
    let worker = fixture("stall", &log, options());
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(matches!(
        worker.call(WorkerRequest::Ping, WAIT, &cancelled).await,
        Err(WorkerError::Cancelled)
    ));
    assert!(
        events(&log, "start").is_empty(),
        "pre-cancelled admission has no process side effect"
    );
    assert!(matches!(
        worker
            .call(
                WorkerRequest::Ping,
                // The stalled worker must launch and log the request before
                // this deadline reaps it, or no process id is left to check.
                Duration::from_secs(1),
                &CancellationToken::new()
            )
            .await,
        Err(WorkerError::Deadline)
    ));
    reaped(&event(&log, "request", 1).await).await;
    assert_eq!(worker.pending(), 0);
    worker.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn dropping_an_active_call_cancels_it_and_allows_a_fresh_worker() {
    let directory = tempfile::tempdir().expect("test directory");
    let log = directory.path().join("worker.log");
    let worker = Arc::new(fixture(
        "stall-once",
        &log,
        WorkerOptions {
            max_strikes: 1,
            ..options()
        },
    ));
    let request = WorkerRequest::Parse {
        path: "cancelled.rs".into(),
        language: "rust".into(),
        source: "fn cancelled() {}".into(),
    };
    let active = {
        let worker = Arc::clone(&worker);
        let request = request.clone();
        tokio::spawn(async move { worker.call(request, WAIT, &CancellationToken::new()).await })
    };
    let pid = event(&log, "request", 1).await[0];
    active.abort();
    assert!(active.await.expect_err("aborted call").is_cancelled());
    pending(&worker, 0).await;
    assert!(
        matches!(
            worker.call(request, WAIT, &CancellationToken::new()).await,
            Ok(WorkerResponse::Pong)
        ),
        "caller cancellation never quarantines the input"
    );
    assert_eq!(worker.status().await.quarantined, 0);
    reaped(&[pid]).await;
    assert_eq!(
        events(&log, "start").len(),
        2,
        "cancelled worker is never reused"
    );
    worker.shutdown().await.expect("shutdown");
    reaped(&events(&log, "start")).await;
}

#[tokio::test]
async fn a_crash_has_restart_backoff_and_the_next_worker_serves_requests() {
    let directory = tempfile::tempdir().expect("test directory");
    let log = directory.path().join("worker.log");
    let backoff = Duration::from_millis(200);
    let worker = fixture(
        "crash-once",
        &log,
        WorkerOptions {
            backoff: vec![backoff],
            ..options()
        },
    );
    assert!(matches!(
        ping(&worker).await,
        Err(WorkerError::Unavailable(_))
    ));
    let failure = Instant::now();
    let cold = magistr_workspace::engine::search("needle", "a needle remains readable", false)
        .expect("cold lexical search remains available after the worker crashes");
    assert_eq!(cold.len(), 1);
    assert!(
        worker.status().await.pid.is_none(),
        "lexical reads do not restart the worker"
    );
    assert!(matches!(ping(&worker).await, Ok(WorkerResponse::Pong)));
    assert!(
        failure.elapsed() >= Duration::from_millis(150),
        "restart observes configured backoff"
    );
    assert_eq!(events(&log, "start").len(), 2);
    assert!(worker.status().await.restarts >= 1);
    worker.shutdown().await.expect("shutdown");
    reaped(&events(&log, "start")).await;
}

#[tokio::test]
async fn repeated_crashes_quarantine_the_input_and_stop_respawning_it() {
    let directory = tempfile::tempdir().expect("test directory");
    let log = directory.path().join("worker.log");
    let worker = fixture(
        "crash",
        &log,
        WorkerOptions {
            max_strikes: 2,
            ..options()
        },
    );
    let request = WorkerRequest::Parse {
        path: "poison.rs".into(),
        language: "rust".into(),
        source: "fn poison() {}".into(),
    };
    for _ in 0..2 {
        assert!(matches!(
            worker
                .call(request.clone(), WAIT, &CancellationToken::new())
                .await,
            Err(WorkerError::Unavailable(_))
        ));
    }
    assert!(matches!(
        worker.call(request, WAIT, &CancellationToken::new()).await,
        Err(WorkerError::Quarantined(_))
    ));
    assert_eq!(
        events(&log, "start").len(),
        2,
        "quarantined input never starts a third worker"
    );
    assert_eq!(worker.status().await.quarantined, 1);
    assert_eq!(worker.pending(), 0);
    let corrected = WorkerRequest::Parse {
        path: "poison.rs".into(),
        language: "rust".into(),
        source: "fn corrected() {}".into(),
    };
    assert!(
        matches!(
            worker
                .call(corrected, WAIT, &CancellationToken::new())
                .await,
            Err(WorkerError::Unavailable(_))
        ),
        "a changed input is admitted to a new process"
    );
    assert_eq!(events(&log, "start").len(), 3);
    worker.shutdown().await.expect("shutdown");
    reaped(&events(&log, "start")).await;
}

#[tokio::test]
async fn quarantine_retains_only_the_configured_number_of_inputs() {
    let directory = tempfile::tempdir().expect("test directory");
    let log = directory.path().join("worker.log");
    let worker = fixture(
        "crash",
        &log,
        WorkerOptions {
            max_strikes: 1,
            max_quarantine_entries: 2,
            ..options()
        },
    );
    for index in 0..5 {
        let request = WorkerRequest::Parse {
            path: format!("poison-{index}.rs"),
            language: "rust".into(),
            source: "fn poison() {}".into(),
        };
        assert!(matches!(
            worker
                .call(request.clone(), WAIT, &CancellationToken::new())
                .await,
            Err(WorkerError::Unavailable(_))
        ));
        assert!(matches!(
            worker.call(request, WAIT, &CancellationToken::new()).await,
            Err(WorkerError::Quarantined(_))
        ));
        assert!(
            worker.status().await.quarantined <= 2,
            "quarantine registry has a fixed ceiling"
        );
    }
    assert_eq!(events(&log, "start").len(), 5);
    worker.shutdown().await.expect("shutdown");
    reaped(&events(&log, "start")).await;
}

#[tokio::test]
async fn native_parser_and_dynamic_grammar_execute_in_the_same_owned_process() {
    use magistr_workspace::worker::WorkerLanguageLane;

    let worker = WorkerSupervisor::new(magistr_binary(), WorkerOptions::default())
        .expect("native supervisor");
    let result = worker
        .call(
            WorkerRequest::Parse {
                path: "main.rs".into(),
                language: "rust".into(),
                source: "fn main() {}".into(),
            },
            WAIT,
            &CancellationToken::new(),
        )
        .await
        .expect("native parse");
    assert!(
        matches!(result, WorkerResponse::Parse { has_error: false, error_count: 0, node_count, .. } if node_count > 0)
    );
    let pid = worker.status().await.pid.expect("native worker pid");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../magistr-workspace/fixtures/plugins/lang-pack-gleam");
    let registration = worker
        .call(
            WorkerRequest::RegisterLanguage {
                id: "gleam".into(),
                extensions: vec!["gleam".into()],
                grammar_wasm: std::fs::read(root.join("grammar/gleam.wasm")).expect("real grammar"),
                tag_queries: std::fs::read_to_string(root.join("queries/tags.scm"))
                    .expect("real queries"),
                blessed: false,
            },
            WAIT,
            &CancellationToken::new(),
        )
        .await
        .expect("register dynamic grammar");
    assert!(
        matches!(registration, WorkerResponse::LanguageRegistered { registration }
        if registration.lane == WorkerLanguageLane::Wasm && registration.queries_ok)
    );
    let result = worker
        .call(
            WorkerRequest::Parse {
                path: "main.gleam".into(),
                language: "gleam".into(),
                source: "pub fn main() { 42 }".into(),
            },
            WAIT,
            &CancellationToken::new(),
        )
        .await
        .expect("dynamic parse");
    assert!(
        matches!(result, WorkerResponse::Parse { has_error: false, error_count: 0, node_count, .. } if node_count > 0)
    );
    assert_eq!(
        worker.status().await.pid,
        Some(pid),
        "worker owns its grammar registry"
    );
    worker.shutdown().await.expect("native shutdown");
    reaped(&[pid]).await;
}

#[tokio::test]
async fn sandbox_limits_and_capability_denials_preserve_the_worker() {
    use magistr_workspace::worker::{WorkerGuestEnd, WorkerPolicy};

    let worker = WorkerSupervisor::new(magistr_binary(), WorkerOptions::default())
        .expect("native supervisor");
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../subsystems/workspace/conformance");
    for (file, export, expected) in [
        ("spin.wat", "spin", WorkerGuestEnd::OutOfFuel),
        ("hog.wat", "hog", WorkerGuestEnd::MemoryCap),
        (
            "observe.wat",
            "run",
            WorkerGuestEnd::CapDenied("observe".into()),
        ),
    ] {
        let result = worker
            .call(
                WorkerRequest::Component {
                    guest: std::fs::read(root.join(file)).expect("real component fixture"),
                    export: export.into(),
                    args: vec![],
                    capabilities: vec![],
                    policy: WorkerPolicy {
                        fuel: 100_000,
                        memory_cap_bytes: 10 * 65_536,
                    },
                },
                WAIT,
                &CancellationToken::new(),
            )
            .await
            .expect("bounded guest result");
        let WorkerResponse::Component { end, fuel_used, .. } = result else {
            panic!("component response expected");
        };
        assert_eq!(end, expected, "{file}");
        assert!(fuel_used <= 100_000, "fuel ceiling: {file}");
    }
    let pid = worker
        .status()
        .await
        .pid
        .expect("worker survives guest failures");
    let allowed = worker
        .call(
            WorkerRequest::Component {
                guest: std::fs::read(root.join("observe.wat")).expect("observe fixture"),
                export: "run".into(),
                args: vec![],
                capabilities: vec!["observe".into()],
                policy: WorkerPolicy {
                    fuel: 100_000,
                    memory_cap_bytes: 10 * 65_536,
                },
            },
            WAIT,
            &CancellationToken::new(),
        )
        .await
        .expect("granted capability");
    assert!(
        matches!(allowed, WorkerResponse::Component { end: WorkerGuestEnd::Ok, observations, .. }
        if observations == [7, 11, 13])
    );
    assert!(matches!(ping(&worker).await, Ok(WorkerResponse::Pong)));
    assert_eq!(worker.status().await.pid, Some(pid));
    worker.shutdown().await.expect("native shutdown");
    reaped(&[pid]).await;
}

#[tokio::test]
async fn shutdown_stops_active_and_queued_calls_and_reaps_the_owned_worker() {
    let directory = tempfile::tempdir().expect("test directory");
    let log = directory.path().join("worker.log");
    let worker = Arc::new(fixture("stall", &log, options()));
    let active = {
        let worker = Arc::clone(&worker);
        tokio::spawn(async move { ping(&worker).await })
    };
    let pid = event(&log, "request", 1).await[0];
    let queued = {
        let worker = Arc::clone(&worker);
        tokio::spawn(async move { ping(&worker).await })
    };
    pending(&worker, 2).await;
    tokio::time::timeout(Duration::from_secs(3), worker.shutdown())
        .await
        .expect("shutdown interrupts stalled work")
        .expect("shutdown");
    assert!(matches!(
        active.await.expect("active task"),
        Err(WorkerError::Stopped)
    ));
    assert!(matches!(
        queued.await.expect("queued task"),
        Err(WorkerError::Stopped)
    ));
    assert!(matches!(ping(&worker).await, Err(WorkerError::Stopped)));
    assert!(worker.status().await.stopped);
    assert_eq!(worker.pending(), 0);
    reaped(&[pid]).await;
    worker.shutdown().await.expect("idempotent shutdown");
}

#[cfg(unix)]
#[tokio::test]
async fn shutdown_cleans_owned_descendants_and_preserves_an_unrelated_process() {
    let directory = tempfile::tempdir().expect("test directory");
    let log = directory.path().join("worker.log");
    let mut unrelated = tokio::process::Command::new("sleep")
        .arg("30")
        .kill_on_drop(true)
        .spawn()
        .expect("independent process");
    let unrelated_pid = unrelated.id().expect("independent pid");
    let worker = Arc::new(fixture("descendant", &log, options()));
    let active = {
        let worker = Arc::clone(&worker);
        tokio::spawn(async move { ping(&worker).await })
    };
    let descendant = event(&log, "descendant", 1).await[0];
    let parent = event(&log, "start", 1).await[0];
    worker.shutdown().await.expect("shutdown process group");
    assert!(matches!(
        active.await.expect("active task"),
        Err(WorkerError::Stopped)
    ));
    reaped(&[parent, descendant]).await;
    assert!(
        workspace_worker_support::is_alive(unrelated_pid),
        "supervisor only signals its own process group"
    );
    unrelated
        .kill()
        .await
        .expect("clean up independent test process");
    unrelated
        .wait()
        .await
        .expect("reap independent test process");
}

/// What a composition installs: every incarnation answers a restore request
/// before the supervisor dispatches the caller's own; `refuse_first` refuses
/// the first incarnation from the restore itself.
struct CountingRestore {
    invoked: std::sync::atomic::AtomicUsize,
    refuse_first: bool,
}

impl magistr_workspace::worker::WorkerRestore for CountingRestore {
    fn restore<'a>(
        &'a self,
        incarnation: &'a mut magistr_workspace::worker::WorkerIncarnation<'_>,
    ) -> magistr_workspace::worker::RestoreFuture<'a> {
        Box::pin(async move {
            let attempt = self
                .invoked
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if self.refuse_first && attempt == 0 {
                return Err(WorkerError::Protocol("restore refused".into()));
            }
            assert!(incarnation.pid().is_some(), "a spawned process");
            match incarnation.call(WorkerRequest::Ping, WAIT).await? {
                WorkerResponse::Pong => Ok(()),
                other => Err(WorkerError::Protocol(format!(
                    "unexpected restore answer {other:?}"
                ))),
            }
        })
    }
}

impl CountingRestore {
    fn invoked(&self) -> usize {
        self.invoked.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[tokio::test]
async fn every_incarnation_is_restored_before_it_serves_and_a_lost_restore_never_admits_it() {
    let directory = tempfile::tempdir().expect("test directory");
    let log = directory.path().join("worker.log");
    // The first process dies on its first request, which is the restore's.
    let worker = fixture("crash-once", &log, options());
    let restore = Arc::new(CountingRestore {
        invoked: std::sync::atomic::AtomicUsize::new(0),
        refuse_first: false,
    });
    worker.install_restore(restore.clone());
    assert!(matches!(
        ping(&worker).await,
        Err(WorkerError::Unavailable(_))
    ));
    assert_eq!(restore.invoked(), 1);
    assert!(
        worker.status().await.pid.is_none(),
        "a process whose restore was lost is never admitted"
    );
    assert!(matches!(ping(&worker).await, Ok(WorkerResponse::Pong)));
    assert_eq!(restore.invoked(), 2, "the replacement is restored itself");
    assert_eq!(events(&log, "start").len(), 2);
    assert_eq!(
        events(&log, "request").len(),
        3,
        "one restore request per incarnation, then the caller's"
    );
    assert!(worker.status().await.restarts >= 1);
    assert!(matches!(ping(&worker).await, Ok(WorkerResponse::Pong)));
    assert_eq!(
        restore.invoked(),
        2,
        "an admitted incarnation is not restored again"
    );
    assert_eq!(events(&log, "request").len(), 4);
    worker.shutdown().await.expect("shutdown");
    reaped(&events(&log, "start")).await;
}

#[tokio::test]
async fn a_restore_that_refuses_an_incarnation_stops_it_and_the_next_call_starts_over() {
    let directory = tempfile::tempdir().expect("test directory");
    let log = directory.path().join("worker.log");
    let worker = fixture("answer", &log, options());
    let restore = Arc::new(CountingRestore {
        invoked: std::sync::atomic::AtomicUsize::new(0),
        refuse_first: true,
    });
    worker.install_restore(restore.clone());
    let error = ping(&worker)
        .await
        .expect_err("refused restore fails the caller explicitly");
    assert_eq!(error, WorkerError::Protocol("restore refused".into()));
    assert!(worker.status().await.pid.is_none());
    let first = event(&log, "start", 1).await;
    reaped(&first).await;
    assert!(
        events(&log, "request").is_empty(),
        "nothing was dispatched to the refused incarnation"
    );
    assert!(matches!(ping(&worker).await, Ok(WorkerResponse::Pong)));
    assert_eq!(restore.invoked(), 2);
    assert_eq!(events(&log, "start").len(), 2);
    assert_ne!(worker.status().await.pid, Some(first[0]));
    worker.shutdown().await.expect("shutdown");
    reaped(&events(&log, "start")).await;
}
