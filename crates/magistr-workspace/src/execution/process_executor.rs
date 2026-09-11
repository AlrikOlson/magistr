use std::{
    path::PathBuf,
    process::Stdio,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::{Child, Command},
};
use tokio_util::sync::CancellationToken;

use super::{RunExecution, RunExecutor, RunFuture, RunLimits, bounded_capture::BoundedCapture};
use crate::contracts::{ErrorKind, WorkspaceError};

/// The production executor: one argv spawned in the workspace root with no
/// shell, stdin closed, both streams captured under the ceiling, and the
/// child in its own process group that this executor alone kills. A timeout
/// or a cancellation kills the group (never a caller-supplied pid), waits,
/// and answers partial output as data.
#[derive(Debug, Clone)]
pub struct ProcessExecutor {
    root: PathBuf,
}

impl ProcessExecutor {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    async fn run(
        &self,
        argv: Vec<String>,
        limits: RunLimits,
        cancel: CancellationToken,
    ) -> Result<RunExecution, WorkspaceError> {
        let (program, args) = argv
            .split_first()
            .ok_or_else(|| WorkspaceError::new(ErrorKind::RunRedirected, "run: empty argv"))?;
        let mut command = Command::new(program);
        command
            .args(args)
            .current_dir(&self.root)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(unix)]
        command.process_group(0);
        let started = Instant::now();
        let mut child = command
            .spawn()
            .map_err(|error| spawn_error(program, &error))?;
        let pid = child.id();
        let stdout = capture(child.stdout.take(), limits.capture_bytes);
        let stderr = capture(child.stderr.take(), limits.capture_bytes);
        let (stdout_buffer, stderr_buffer) = (stdout.0.clone(), stderr.0.clone());
        let mut readers = tokio::spawn(async move {
            tokio::join!(stdout.1, stderr.1);
        });
        let deadline = tokio::time::sleep(limits.timeout);
        tokio::pin!(deadline);
        let mut execution = RunExecution {
            pid,
            ..RunExecution::default()
        };
        let status = tokio::select! {
            status = child.wait() => Some(status),
            () = &mut deadline => { execution.timed_out = true; None }
            () = cancel.cancelled() => { execution.cancelled = true; None }
        };
        let mut drained = false;
        match status {
            Some(status) => {
                let status = status.map_err(|error| {
                    WorkspaceError::new(ErrorKind::EngineUnavailable, format!("run: wait: {error}"))
                })?;
                execution.exit_code = status.code();
                #[cfg(unix)]
                {
                    use std::os::unix::process::ExitStatusExt;
                    execution.signal = status.signal();
                }
                // A grandchild holding the pipes must not hang the answer:
                // drain until the deadline or the cancel, then kill the group.
                tokio::select! {
                    _ = &mut readers => { drained = true; }
                    () = &mut deadline => { execution.timed_out = true; kill_group(&mut child, pid).await; }
                    () = cancel.cancelled() => { execution.cancelled = true; kill_group(&mut child, pid).await; }
                }
            }
            None => kill_group(&mut child, pid).await,
        }
        // The group is dead or exited; the pipes close promptly. Bound anyway.
        if !drained
            && tokio::time::timeout(Duration::from_secs(2), &mut readers)
                .await
                .is_err()
        {
            readers.abort();
        }
        execution.duration_ms = started.elapsed().as_millis() as u64;
        let (out, err) = (lock(&stdout_buffer), lock(&stderr_buffer));
        execution.stdout = out.text();
        execution.stderr = err.text();
        execution.dropped_bytes = out.dropped() + err.dropped();
        Ok(execution)
    }
}

impl RunExecutor for ProcessExecutor {
    fn execute(
        &self,
        argv: Vec<String>,
        limits: RunLimits,
        cancel: CancellationToken,
    ) -> RunFuture<'_, Result<RunExecution, WorkspaceError>> {
        Box::pin(self.run(argv, limits, cancel))
    }
}

type Shared = Arc<Mutex<BoundedCapture>>;

fn lock(capture: &Shared) -> std::sync::MutexGuard<'_, BoundedCapture> {
    capture
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// A shared bounded buffer plus the pump that fills it; partial output stays
/// readable even when the pump is abandoned after a kill.
fn capture<S: AsyncRead + Unpin + Send + 'static>(
    stream: Option<S>,
    capture_bytes: usize,
) -> (Shared, RunFuture<'static, ()>) {
    let shared = Arc::new(Mutex::new(BoundedCapture::new(capture_bytes)));
    let sink = shared.clone();
    let pump: RunFuture<'static, ()> = Box::pin(async move {
        let Some(mut stream) = stream else {
            return;
        };
        let mut buffer = vec![0_u8; 16 * 1024];
        loop {
            match stream.read(&mut buffer).await {
                Ok(0) | Err(_) => return,
                Ok(count) => lock(&sink).push(&buffer[..count]),
            }
        }
    });
    (shared, pump)
}

fn spawn_error(program: &str, error: &std::io::Error) -> WorkspaceError {
    if error.kind() == std::io::ErrorKind::NotFound {
        WorkspaceError::new(
            ErrorKind::FileNotFound,
            format!(
                "run: the declared program {program:?} could not be found on PATH or at that path; install it or amend the declared [commands] line"
            ),
        )
    } else {
        WorkspaceError::new(
            ErrorKind::EngineUnavailable,
            format!("run: could not start {program:?}: {error}"),
        )
    }
}

/// Kill the dedicated group this executor created, then reap the leader. The
/// group id is the spawned child's own pid — never a caller-provided value.
async fn kill_group(child: &mut Child, pid: Option<u32>) {
    #[cfg(unix)]
    if let Some(group) = pid.and_then(|pid| i32::try_from(pid).ok())
        && group > 0
    {
        // SAFETY: spawn placed the child in its own process group whose id is
        // its pid, and the leader is still owned and unreaped by `child`.
        unsafe {
            libc::kill(-group, libc::SIGKILL);
        }
    }
    #[cfg(not(unix))]
    let _ = pid;
    let _ = child.start_kill();
    let _ = child.wait().await;
}

#[cfg(test)]
#[path = "process_executor.tests.rs"]
mod tests;
