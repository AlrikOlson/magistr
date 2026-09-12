use std::path::PathBuf;

use magistr_process::{Launch, OwnedProcess, ProcessSpec, Termination};
use tokio_util::sync::CancellationToken;

use super::{RunExecution, RunExecutor, RunFuture, RunLimits};
use crate::contracts::{ErrorKind, WorkspaceError};

/// The production executor: one admitted argv spawned in the workspace root
/// with no shell through Magistr's owned-process mechanism — stdin closed,
/// both streams captured under the ceiling, the child in its own process
/// group that the mechanism alone ends. A timeout or a cancellation kills
/// the group at once (never a caller-supplied pid), waits, and answers
/// partial output as data. The argv is the only launch this executor
/// builds: a shell line never reaches it.
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
        let program = argv
            .first()
            .cloned()
            .ok_or_else(|| WorkspaceError::new(ErrorKind::RunRedirected, "run: empty argv"))?;
        let spec = ProcessSpec::new(Launch::Argv(argv), limits.timeout)
            .in_dir(&self.root)
            .with_capture_bytes(limits.capture_bytes)
            .with_termination(Termination::Immediate);
        let process = OwnedProcess::spawn(spec).map_err(|error| spawn_error(&program, &error))?;
        let outcome = process.run(&cancel, |_| {}).await;
        if let Some(fault) = outcome.fault {
            return Err(WorkspaceError::new(
                ErrorKind::EngineUnavailable,
                format!("run: {fault}"),
            ));
        }
        Ok(RunExecution {
            exit_code: outcome.exit_code,
            signal: outcome.signal,
            stdout: outcome.stdout,
            stderr: outcome.stderr,
            timed_out: outcome.timed_out,
            cancelled: outcome.cancelled,
            duration_ms: outcome.duration.as_millis() as u64,
            dropped_bytes: outcome.dropped_bytes,
            pid: outcome.pid,
        })
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

#[cfg(test)]
#[path = "process_executor.tests.rs"]
mod tests;
