//! Real LSP children beneath the native Magistr worker and Wasmtime resolver.

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use magistr_workspace::worker::{
    WorkerError, WorkerGuestEnd, WorkerOptions, WorkerPolicy, WorkerRequest, WorkerResponse,
    WorkerSubprocessSpec, WorkerSupervisor,
};
use tokio_util::sync::CancellationToken;

const WAIT: Duration = Duration::from_secs(20);
const GUEST: &[u8] =
    include_bytes!("../../magistr-workspace/fixtures/plugins/lsp-resolver/guest.wasm");

fn fixture_binary() -> PathBuf {
    static BINARY: OnceLock<(tempfile::TempDir, PathBuf)> = OnceLock::new();
    BINARY
        .get_or_init(|| {
            let directory = tempfile::tempdir().expect("fixture directory");
            let binary = directory
                .path()
                .join(format!("lsp-fixture{}", std::env::consts::EXE_SUFFIX));
            let source = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/workspace_worker_lsp_support/fixture.rs");
            let result = std::process::Command::new("rustc")
                .args(["--edition=2024", "--crate-name=workspace_lsp_fixture"])
                .arg(source)
                .arg("-o")
                .arg(&binary)
                .output()
                .expect("compile LSP fixture");
            assert!(
                result.status.success(),
                "fixture compiler: {}",
                String::from_utf8_lossy(&result.stderr)
            );
            (directory, binary)
        })
        .1
        .clone()
}

fn request(log: &Path, mode: &str, granted: bool) -> WorkerRequest {
    WorkerRequest::Resolver {
        guest: GUEST.to_vec(),
        symbol: "target".to_owned(),
        capabilities: if granted {
            vec!["magistr:subprocess/subprocess@0.2.0".to_owned()]
        } else {
            vec![]
        },
        policy: WorkerPolicy {
            fuel: 50_000_000,
            memory_cap_bytes: 64 * 1024 * 1024,
        },
        subprocess_specs: vec![WorkerSubprocessSpec {
            language_id: "fixture".to_owned(),
            bin: fixture_binary().to_string_lossy().into_owned(),
            args: vec![log.to_string_lossy().into_owned(), mode.to_owned()],
        }],
        subprocess_root: "/workspace".to_owned(),
    }
}

fn worker() -> WorkerSupervisor {
    WorkerSupervisor::new(
        PathBuf::from(env!("CARGO_BIN_EXE_magistr")),
        WorkerOptions::default(),
    )
    .expect("native supervisor")
}

fn events(log: &Path, event: &str) -> Vec<u32> {
    std::fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| {
            let (pid, found) = line.split_once(' ')?;
            (found == event).then(|| pid.parse().expect("fixture pid"))
        })
        .collect()
}

async fn observed_request(log: &Path) -> u32 {
    tokio::time::timeout(WAIT, async {
        loop {
            if let Some(pid) = events(log, "request").first() {
                return *pid;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("LSP received a real guest request")
}

#[cfg(unix)]
async fn reaped(pids: &[u32]) {
    tokio::time::timeout(WAIT, async {
        loop {
            let alive = pids.iter().any(|pid| {
                // SAFETY: signal zero observes a PID created by this test.
                unsafe { libc::kill(*pid as i32, 0) == 0 }
            });
            if !alive {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("owned worker/LSP processes remain: {pids:?}"));
}

#[cfg(not(unix))]
async fn reaped(_pids: &[u32]) {}

#[tokio::test]
async fn resolver_capabilities_pooling_and_shutdown_cross_the_real_worker_boundary() {
    let directory = tempfile::tempdir().expect("test workspace");
    let log = directory.path().join("lsp.log");
    let worker = worker();
    let denied = worker
        .call(
            request(&log, "echo", false),
            WAIT,
            &CancellationToken::new(),
        )
        .await
        .expect("typed denial");
    assert!(matches!(
        denied,
        WorkerResponse::Resolver {
            end: WorkerGuestEnd::CapDenied(_),
            ..
        }
    ));
    assert!(
        events(&log, "start").is_empty(),
        "ungranted subprocess import cannot spawn"
    );
    for reused in [false, true] {
        let result = worker
            .call(request(&log, "echo", true), WAIT, &CancellationToken::new())
            .await
            .expect("native resolver call");
        let WorkerResponse::Resolver {
            end,
            locations,
            sessions_opened,
            sessions_reused,
            ..
        } = result
        else {
            panic!("resolver response expected");
        };
        assert_eq!(end, WorkerGuestEnd::Ok);
        assert_eq!(
            (sessions_opened, sessions_reused),
            if reused { (0, 1) } else { (1, 0) }
        );
        assert_eq!(locations.len(), 1);
        assert_eq!(
            (&*locations[0].path, locations[0].line, locations[0].column),
            ("lib.rs", 5, 3)
        );
    }
    let pids = events(&log, "start");
    assert_eq!(
        pids.len(),
        1,
        "second invoke must reuse its admitted LSP process"
    );
    let worker_pid = worker.status().await.pid.expect("native worker pid");
    worker.shutdown().await.expect("native shutdown");
    reaped(&[worker_pid, pids[0]]).await;
}

#[tokio::test]
async fn cancellation_reaps_a_real_lsp_while_its_guest_is_blocked_in_host_io() {
    let directory = tempfile::tempdir().expect("test workspace");
    let log = directory.path().join("lsp.log");
    let worker = Arc::new(worker());
    assert!(matches!(
        worker
            .call(WorkerRequest::Ping, WAIT, &CancellationToken::new())
            .await,
        Ok(WorkerResponse::Pong)
    ));
    let worker_pid = worker.status().await.pid.expect("native worker pid");
    let cancel = CancellationToken::new();
    let future_worker = Arc::clone(&worker);
    let future_cancel = cancel.clone();
    let call = request(&log, "stall", true);
    let running = tokio::spawn(async move { future_worker.call(call, WAIT, &future_cancel).await });
    let lsp_pid = observed_request(&log).await;
    cancel.cancel();
    assert!(matches!(
        running.await.expect("caller joined"),
        Err(WorkerError::Cancelled)
    ));
    reaped(&[worker_pid, lsp_pid]).await;
    worker
        .shutdown()
        .await
        .expect("shutdown after cancellation");
}
