use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use magistr_workspace::worker::{WorkerOptions, WorkerResponse, WorkerSupervisor};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub(super) const WAIT: Duration = Duration::from_secs(10);

/// One fixture executable per source and compiler, shared by every test
/// process. Each nextest process compiling its own copy into a fresh directory
/// made macOS scan a new binary on its first launch, often for longer than the
/// deadlines these tests measure, so a stalled worker could be killed before
/// it logged the event the test waits for.
fn fixture_binary() -> PathBuf {
    static BINARY: OnceLock<PathBuf> = OnceLock::new();
    BINARY.get_or_init(build_shared_fixture).clone()
}

fn build_shared_fixture() -> PathBuf {
    let source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/workspace_worker_support/fixture.rs");
    let directory = Path::new(env!("CARGO_TARGET_TMPDIR")).join("workspace-worker-fixture");
    std::fs::create_dir_all(&directory).expect("fixture build directory");
    let binary = directory.join(format!("worker-fixture-{:016x}", fixture_key(&source)));
    if binary.is_file() {
        return binary;
    }
    let lock = std::fs::File::create(directory.join(".build.lock")).expect("fixture build lock");
    lock.lock().expect("lock fixture build");
    if binary.is_file() {
        return binary;
    }
    let partial = directory.join(format!(".worker-fixture-{}", std::process::id()));
    let result = std::process::Command::new("rustc")
        .args(["--edition=2024", "--crate-name=workspace_worker_fixture"])
        .arg(&source)
        .arg("-o")
        .arg(&partial)
        .output()
        .expect("compile standalone worker fixture with the project toolchain");
    assert!(
        result.status.success(),
        "fixture compiler: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    // The first launch of a new executable pays the platform's security scan.
    // Pay it here, before any test starts timing a worker.
    let probe = std::process::Command::new(&partial)
        .arg("probe")
        .status()
        .expect("launch the fixture once");
    assert!(probe.success(), "fixture probe: {probe}");
    // Publish complete bytes only; a racing reader never runs a partial file.
    std::fs::rename(&partial, &binary).expect("publish fixture executable");
    binary
}

/// The fixture source and the compiler that builds it name one executable.
fn fixture_key(source: &Path) -> u64 {
    use std::hash::{Hash, Hasher};
    let compiler = std::process::Command::new("rustc")
        .arg("-vV")
        .output()
        .expect("query the project toolchain");
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    std::fs::read(source)
        .expect("fixture source")
        .hash(&mut hasher);
    compiler.stdout.hash(&mut hasher);
    hasher.finish()
}

pub(super) fn fixture(mode: &str, log: &Path, mut options: WorkerOptions) -> WorkerSupervisor {
    options.launch_args = vec![
        mode.into(),
        log.as_os_str().to_owned(),
        serde_json::to_string(&WorkerResponse::Pong)
            .expect("pong serialization")
            .into(),
    ];
    WorkerSupervisor::new(fixture_binary(), options).expect("fixture supervisor")
}

pub(super) fn options() -> WorkerOptions {
    WorkerOptions {
        backoff: vec![Duration::from_millis(25)],
        ready_timeout: Duration::from_secs(3),
        ..WorkerOptions::default()
    }
}

pub(super) fn events(log: &Path, event: &str) -> Vec<u32> {
    std::fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| {
            let (pid, found) = line.split_once(' ')?;
            (found == event).then(|| pid.parse().expect("logged process id"))
        })
        .collect()
}

pub(super) async fn event(log: &Path, name: &str, count: usize) -> Vec<u32> {
    tokio::time::timeout(WAIT, async {
        loop {
            let found = events(log, name);
            if found.len() >= count {
                return found;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("fixture event before timeout")
}

pub(super) async fn pending(supervisor: &WorkerSupervisor, count: usize) {
    tokio::time::timeout(WAIT, async {
        while supervisor.pending() != count {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("expected admission count before timeout");
}

pub(super) async fn read_frame(stdout: &mut tokio::process::ChildStdout) -> serde_json::Value {
    tokio::time::timeout(WAIT, async {
        let size = stdout.read_u32_le().await.expect("worker frame header") as usize;
        assert!(size <= 32 * 1024 * 1024, "bounded response frame");
        let mut body = vec![0; size];
        stdout
            .read_exact(&mut body)
            .await
            .expect("worker frame body");
        serde_json::from_slice(&body).expect("worker JSON frame")
    })
    .await
    .expect("worker response before timeout")
}

pub(super) async fn write_frame(stdin: &mut tokio::process::ChildStdin, bytes: &[u8]) {
    stdin
        .write_u32_le(bytes.len().try_into().expect("bounded test frame"))
        .await
        .expect("write frame header");
    stdin.write_all(bytes).await.expect("write frame body");
    stdin.flush().await.expect("flush frame");
}

#[cfg(unix)]
pub(super) fn is_alive(pid: u32) -> bool {
    // SAFETY: signal 0 observes this test's known child PID without signalling it.
    let result = unsafe { libc::kill(pid as i32, 0) };
    result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

pub(super) async fn reaped(pids: &[u32]) {
    #[cfg(unix)]
    tokio::time::timeout(WAIT, async {
        while pids.iter().copied().any(is_alive) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("owned worker processes still exist: {pids:?}"));
    #[cfg(not(unix))]
    let _ = pids;
}
