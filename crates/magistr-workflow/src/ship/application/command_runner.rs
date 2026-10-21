//! Asynchronous command execution outside the native state lock.

use std::path::Path;
use std::time::Duration;

use magistr_process::{Launch, OwnedProcess, ProcessSpec, Termination};
use tokio_util::sync::CancellationToken;

use super::{CheckAdmission, CheckExecution, CheckOutcome, ShipUseCases};

// ── Verified-check command execution ───────────────────────────────

/// Result of running a `ship_check` command.
pub(crate) struct CommandOutcome {
    pub(crate) passed: bool,
    pub(crate) exit_code: Option<i32>,
    pub(crate) execution: CheckExecution,
    pub(crate) output_tail: String,
}

/// Max chars of command output retained in the check `details`.
pub(crate) const CHECK_OUTPUT_TAIL: usize = 4000;
/// A verified check command is killed after this long so a hung gate can't pin
/// a slot forever.
///
/// Public because [`crate::mcp::tasks::TASK_TTL_MS`] is *derived* from it: a
/// task retention window shorter than the longest a gate may run would mark a
/// still-running gate `failed`.
pub(crate) const CHECK_TIMEOUT_SECS: u64 = super::CheckOptions::MAX_TIMEOUT_SECS;
/// How long the group gets after `SIGTERM` before `SIGKILL` on a timeout.
const KILL_GRACE: Duration = Duration::from_secs(5);

/// Run a check command in its admitted workspace, when source was observed,
/// capturing exit code + a tail of combined stdout/stderr. The command is the
/// caller's own string and runs through the platform shell so `&&`, pipes,
/// and args work as written — Magistr's owned-process mechanism supplies the
/// group, the cap and the kill; it never grants the line the workspace's
/// declared-command admission. Dropping the future mid-run kills the child.
#[cfg(test)]
pub(crate) async fn run_check_command(cmd: &str, cwd: Option<&Path>) -> CommandOutcome {
    run_check_command_with_timeout(
        cmd,
        cwd,
        super::CheckOptions::DEFAULT_TIMEOUT_SECS,
        &Default::default(),
    )
    .await
}

async fn run_check_command_with_timeout(
    cmd: &str,
    cwd: Option<&Path>,
    timeout_secs: u64,
    schedule: &magistr_process::GateSchedule,
) -> CommandOutcome {
    let started = std::time::Instant::now();
    let cancel = CancellationToken::new();
    let root = cwd
        .map(Path::to_path_buf)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_default();
    let _permit = match magistr_process::acquire_gate(
        &root,
        cmd,
        schedule,
        Duration::from_secs(timeout_secs),
        &cancel,
    )
    .await
    {
        Ok(permit) => permit,
        Err(_) => {
            return CommandOutcome {
                passed: false,
                exit_code: None,
                execution: CheckExecution::TimedOut,
                output_tail: format!(
                    "`{cmd}` exceeded its {timeout_secs}s deadline waiting for verification resources"
                ),
            };
        }
    };
    let queued = started.elapsed();
    let mut spec = ProcessSpec::new(
        Launch::Shell(cmd.to_owned()),
        Duration::from_secs(timeout_secs).saturating_sub(queued),
    )
    .with_termination(Termination::Graceful { grace: KILL_GRACE });
    spec.cwd = cwd.map(Path::to_path_buf);
    let process = match OwnedProcess::spawn(spec) {
        Ok(process) => process,
        Err(error) => return launch_failed(cmd, &error.to_string()),
    };
    let outcome = process.run(&CancellationToken::new(), |_| {}).await;
    if let Some(fault) = &outcome.fault {
        return launch_failed(cmd, fault);
    }
    if outcome.timed_out {
        return CommandOutcome {
            passed: false,
            exit_code: None,
            execution: CheckExecution::TimedOut,
            output_tail: format!("`{cmd}` timed out after {timeout_secs}s"),
        };
    }
    let code = outcome.exit_code;
    let mut combined = outcome.stdout;
    if !outcome.stderr.trim().is_empty() {
        combined.push_str(&outcome.stderr);
    }
    let code_str = code.map_or_else(|| "signal".to_string(), |c| c.to_string());
    CommandOutcome {
        passed: code == Some(0),
        exit_code: code,
        // A process ended by a signal has no exit code; it did not
        // pass, and -1 is the code the host's own runner reports.
        execution: CheckExecution::Exited {
            code: code.unwrap_or(-1),
        },
        output_tail: format!(
            "$ {cmd}\n[exit {code_str}; queue {}ms; execution {}ms]\n{}",
            queued.as_millis(),
            started.elapsed().saturating_sub(queued).as_millis(),
            tail_chars(&combined)
        ),
    }
}

fn launch_failed(cmd: &str, reason: &str) -> CommandOutcome {
    CommandOutcome {
        passed: false,
        exit_code: None,
        execution: CheckExecution::LaunchFailed {
            reason: reason.to_owned(),
        },
        output_tail: format!("failed to run `{cmd}`: {reason}"),
    }
}

/// Keep the last [`CHECK_OUTPUT_TAIL`] chars of `s` (failures show up at the end
/// of build/test output), prefixed with an elision marker when trimmed.
pub(crate) fn tail_chars(s: &str) -> String {
    let s = s.trim_end();
    let n = s.chars().count();
    if n <= CHECK_OUTPUT_TAIL {
        return s.to_string();
    }
    let tail: String = s.chars().skip(n - CHECK_OUTPUT_TAIL).collect();
    format!("…(truncated {} chars)…\n{tail}", n - CHECK_OUTPUT_TAIL)
}

impl ShipUseCases {
    pub async fn execute_check(admission: &CheckAdmission) -> CheckOutcome {
        let request = &admission.request;
        let cwd = admission
            .source
            .as_ref()
            .map(|source| Path::new(&source.workspace_root));
        let (passed, details, verified, command, exit_code, execution) = match &request.command {
            Some(command) => {
                let outcome = run_check_command_with_timeout(
                    command,
                    cwd,
                    request
                        .options
                        .timeout_secs
                        .unwrap_or(super::CheckOptions::DEFAULT_TIMEOUT_SECS),
                    &magistr_process::GateSchedule {
                        max_parallel: request.options.max_parallel.unwrap_or(4),
                        resources: request.options.resources.clone(),
                    },
                )
                .await;
                let details = if request.details.is_empty() {
                    outcome.output_tail
                } else {
                    format!("{}\n{}", request.details, outcome.output_tail)
                };
                (
                    outcome.passed,
                    details,
                    true,
                    Some(command.clone()),
                    outcome.exit_code,
                    outcome.execution,
                )
            }
            None => (
                request.passed.unwrap_or(false),
                request.details.clone(),
                false,
                None,
                None,
                ShipUseCases::self_reported(request),
            ),
        };
        let (report, results) = match &request.report {
            Some((format, path)) => {
                let path =
                    cwd.map_or_else(|| Path::new(path).to_path_buf(), |root| root.join(path));
                let (record, results) =
                    crate::ship::report::read_report(format, &path.to_string_lossy());
                (Some(record), results)
            }
            None => (None, None),
        };
        CheckOutcome {
            passed,
            details,
            verified,
            command,
            exit_code,
            execution,
            report,
            results,
        }
    }
}

#[cfg(test)]
#[path = "command_runner.tests.rs"]
mod tests;
