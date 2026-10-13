use std::io::Write;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::Path;
use std::process::{Command as StdCommand, Stdio};
use std::time::Duration;

use tokio_util::sync::CancellationToken;
use windows_sys::Win32::Foundation::WAIT_OBJECT_0;
use windows_sys::Win32::System::Threading::{
    OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject,
};

use super::*;

const FIXTURE: &str = "owned_process::windows_tests::child_fixture";
const DIRECTORY: &str = "MAGISTR_PROCESS_FIXTURE_DIR";
const ROLE: &str = "MAGISTR_PROCESS_FIXTURE_ROLE";

struct FixtureProcess(std::process::Child);
impl Drop for FixtureProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn fixture(dir: &Path, role: &str) -> StdCommand {
    let mut command = StdCommand::new(std::env::current_exe().expect("test executable"));
    command
        .args(["--ignored", "--exact", FIXTURE, "--nocapture"])
        .env(DIRECTORY, dir)
        .env(ROLE, role);
    command
}

fn tree(dir: &Path, role: &str, millis: u64) -> OwnedProcess {
    let executable = std::env::current_exe().expect("test executable");
    let spec = ProcessSpec::new(
        Launch::Argv(vec![
            executable.to_string_lossy().into_owned(),
            "--ignored".into(),
            "--exact".into(),
            FIXTURE.into(),
            "--nocapture".into(),
        ]),
        Duration::from_millis(millis),
    )
    .with_env([
        (DIRECTORY.into(), dir.to_string_lossy().into_owned()),
        (ROLE.into(), role.into()),
    ]);
    OwnedProcess::spawn(spec).expect("contained fixture")
}

async fn observe(dir: &Path, roles: &[&str]) -> Vec<OwnedHandle> {
    let mut handles = Vec::new();
    for role in roles {
        let path = dir.join(format!("{role}.pid"));
        let started = std::time::Instant::now();
        let pid = loop {
            if let Ok(text) = std::fs::read_to_string(&path)
                && let Ok(pid) = text.parse::<u32>()
            {
                break pid;
            }
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "fixture missing: {path:?}"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        };
        // SAFETY: retain a wait-only handle to this fixture, avoiding pid reuse.
        let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
        assert!(!handle.is_null(), "fixture process {pid}");
        // SAFETY: successful OpenProcess transfers ownership of this handle.
        handles.push(unsafe { OwnedHandle::from_raw_handle(handle) });
    }
    handles
}

async fn gone(handles: &[OwnedHandle]) {
    let started = std::time::Instant::now();
    loop {
        // SAFETY: retained wait-only handles remain valid after process exit.
        if handles
            .iter()
            .all(|h| unsafe { WaitForSingleObject(h.as_raw_handle(), 0) } == WAIT_OBJECT_0)
        {
            return;
        }
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "owned descendants survived"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[test]
#[ignore = "subprocess fixture, invoked only by ownership controls"]
fn child_fixture() {
    let dir = std::path::PathBuf::from(std::env::var_os(DIRECTORY).expect("fixture directory"));
    let role = std::env::var(ROLE).expect("fixture role");
    std::fs::write(
        dir.join(format!("{role}.pid")),
        std::process::id().to_string(),
    )
    .expect("pid");
    if role == "host" {
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        runtime.block_on(async {
            tree(&dir, "parent", 30_000)
                .run(&CancellationToken::new(), |_| {})
                .await;
        });
        return;
    }
    let next = match role.as_str() {
        "parent" | "exit_parent" => Some("child"),
        "child" => Some("grandchild"),
        _ => None,
    };
    let mut child = next.map(|next| fixture(&dir, next).spawn().expect("descendant"));
    std::io::stdout()
        .write_all(b"fixture ready\n")
        .expect("stdout");
    if role == "exit_parent" {
        return;
    }
    std::thread::sleep(Duration::from_secs(30));
    if let Some(child) = child.as_mut() {
        let _ = child.wait();
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn timeout_ends_all_descendants_and_preserves_unrelated_work() {
    let dir = tempfile::tempdir().expect("directory");
    let other = tempfile::tempdir().expect("unrelated directory");
    let mut unrelated = FixtureProcess(
        fixture(other.path(), "unrelated")
            .stdout(Stdio::null())
            .spawn()
            .expect("unrelated"),
    );
    let process = tree(dir.path(), "parent", 2_000);
    let handles = observe(dir.path(), &["parent", "child", "grandchild"]).await;
    let outcome = process.run(&CancellationToken::new(), |_| {}).await;
    let alive = unrelated.0.try_wait().expect("unrelated status").is_none();
    drop(unrelated);
    assert!(alive, "cleanup killed unrelated work");
    assert!(outcome.timed_out && !outcome.cancelled, "{outcome:?}");
    assert!(outcome.fault.is_none(), "{outcome:?}");
    assert_eq!(outcome.exit_code, None);
    assert!(outcome.stdout.contains("fixture ready"));
    gone(&handles).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn cancel_ends_the_job_and_reports_cancellation() {
    let dir = tempfile::tempdir().expect("directory");
    let process = tree(dir.path(), "parent", 30_000);
    let handles = observe(dir.path(), &["parent", "child", "grandchild"]).await;
    let cancel = CancellationToken::new();
    cancel.cancel();
    let result = process.run(&cancel, |_| {}).await;
    assert!(result.cancelled && !result.timed_out, "{result:?}");
    assert!(result.fault.is_none(), "{result:?}");
    gone(&handles).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn dropping_before_run_ends_the_job() {
    let dir = tempfile::tempdir().expect("directory");
    let process = tree(dir.path(), "parent", 30_000);
    let handles = observe(dir.path(), &["parent", "child", "grandchild"]).await;
    drop(process);
    gone(&handles).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn dropping_the_running_future_ends_the_job() {
    let dir = tempfile::tempdir().expect("directory");
    let process = tree(dir.path(), "parent", 30_000);
    let task = tokio::spawn(async move { process.run(&CancellationToken::new(), |_| {}).await });
    let handles = observe(dir.path(), &["parent", "child", "grandchild"]).await;
    task.abort();
    assert!(task.await.expect_err("cancelled future").is_cancelled());
    gone(&handles).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn exited_parent_does_not_hide_live_pipe_holding_descendants() {
    let dir = tempfile::tempdir().expect("directory");
    let process = tree(dir.path(), "exit_parent", 2_000);
    let handles = observe(dir.path(), &["child", "grandchild"]).await;
    let result = process.run(&CancellationToken::new(), |_| {}).await;
    assert_eq!(result.exit_code, Some(0));
    assert!(
        result.timed_out,
        "open descendant pipes must not count as completion"
    );
    assert!(result.fault.is_none(), "{result:?}");
    gone(&handles).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn killing_the_host_closes_the_job_and_kills_descendants() {
    let dir = tempfile::tempdir().expect("directory");
    let mut host = FixtureProcess(
        fixture(dir.path(), "host")
            .stdout(Stdio::null())
            .spawn()
            .expect("host"),
    );
    let handles = observe(dir.path(), &["parent", "child", "grandchild"]).await;
    host.0.kill().expect("abrupt host exit");
    host.0.wait().expect("host reaped");
    gone(&handles).await;
}
