use super::{WorkerError, WorkerOptions, frame::Frame};
use std::{path::Path, process::Stdio, time::Duration};
use tokio::{
    io::AsyncWriteExt,
    process::{Child, ChildStdin, ChildStdout, Command},
};

/// Owns only the process it spawned and that process's dedicated group.
pub(super) struct WorkerProcess {
    child: Child,
    pub(super) stdin: ChildStdin,
    pub(super) stdout: ChildStdout,
    #[cfg(unix)]
    group: i32,
}

impl WorkerProcess {
    pub(super) fn spawn(executable: &Path, options: &WorkerOptions) -> Result<Self, WorkerError> {
        let mut command = Command::new(executable);
        command
            .args(&options.launch_args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        #[cfg(unix)]
        command.process_group(0);
        let mut child = command
            .spawn()
            .map_err(|e| WorkerError::Unavailable(e.to_string()))?;
        #[cfg(unix)]
        let group = child
            .id()
            .and_then(|id| i32::try_from(id).ok())
            .ok_or_else(|| WorkerError::Unavailable("spawned worker has no process id".into()))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| WorkerError::Unavailable("missing worker stdin".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| WorkerError::Unavailable("missing worker stdout".into()))?;
        Ok(Self {
            child,
            stdin,
            stdout,
            #[cfg(unix)]
            group,
        })
    }

    pub(super) fn pid(&self) -> Option<u32> {
        self.child.id()
    }

    pub(super) async fn request(
        &mut self,
        bytes: &[u8],
        id: u64,
    ) -> Result<super::WorkerResponse, WorkerError> {
        self.stdin
            .write_all(bytes)
            .await
            .map_err(|e| WorkerError::Unavailable(e.to_string()))?;
        self.stdin
            .flush()
            .await
            .map_err(|e| WorkerError::Unavailable(e.to_string()))?;
        match Frame::read(&mut self.stdout).await? {
            Frame::Response {
                id: actual,
                response,
                ..
            } if actual == id => Ok(response),
            Frame::Error {
                id: actual, error, ..
            } if actual == id => Err(error),
            _ => Err(WorkerError::Protocol(
                "unexpected response kind or request id".into(),
            )),
        }
    }

    pub(super) async fn stop(&mut self) -> Result<(), WorkerError> {
        // Kill the dedicated group while its leader is still owned and unreaped.
        // No lock file or arbitrary PID is ever consulted.
        self.kill_group();
        #[cfg(unix)]
        {
            self.group = 0;
        }
        let _ = self.child.start_kill();
        self.child
            .wait()
            .await
            .map_err(|e| WorkerError::Unavailable(e.to_string()))?;
        Ok(())
    }

    pub(super) async fn graceful_stop(&mut self) -> Result<(), WorkerError> {
        // Cooperative shutdown lets the sandbox join/reap LSP children first.
        let _ = tokio::time::timeout(Duration::from_millis(500), async {
            Frame::Shutdown {
                v: Frame::version(),
            }
            .write(&mut self.stdin)
            .await?;
            self.stdin
                .shutdown()
                .await
                .map_err(|e| WorkerError::Unavailable(e.to_string()))?;
            // Reading EOF does not reap/release the leader's identity.
            let _ = Frame::read(&mut self.stdout).await;
            Ok::<(), WorkerError>(())
        })
        .await;
        self.stop().await
    }

    fn kill_group(&self) {
        #[cfg(unix)]
        if self.group > 0 {
            // SAFETY: spawn created this dedicated process group; its leader has
            // not been reaped. Never a caller-provided PID or workspace owner.
            unsafe {
                libc::kill(-self.group, libc::SIGKILL);
            }
        }
    }
}

impl Drop for WorkerProcess {
    fn drop(&mut self) {
        self.kill_group();
        // kill_on_drop is a last-resort guard; explicit shutdown also waits.
    }
}
