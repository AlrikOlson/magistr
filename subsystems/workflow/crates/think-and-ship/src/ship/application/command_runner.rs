//! Asynchronous command execution outside the native state lock.

use super::{CheckAdmission, CheckOutcome, ShipUseCases};

// ── Verified-check command execution ───────────────────────────────

/// Result of running a `ship_check` command.
pub(crate) struct CommandOutcome {
    pub(crate) passed: bool,
    pub(crate) exit_code: Option<i32>,
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
pub(crate) const CHECK_TIMEOUT_SECS: u64 = 900;

/// Run a check command in the server's working directory (the project dir),
/// capturing exit code + a tail of combined stdout/stderr. The command runs
/// through the platform shell so `&&`, pipes, and args work as written.
pub(crate) async fn run_check_command(cmd: &str) -> CommandOutcome {
    use std::process::Stdio;
    use tokio::process::Command;

    #[cfg(windows)]
    let (shell, flag) = ("cmd", "/C");
    #[cfg(not(windows))]
    let (shell, flag) = ("sh", "-c");

    let mut command = Command::new(shell);
    command
        .arg(flag)
        .arg(cmd)
        .stdin(Stdio::null())
        .kill_on_drop(true);

    let timeout = std::time::Duration::from_secs(CHECK_TIMEOUT_SECS);
    match tokio::time::timeout(timeout, command.output()).await {
        Ok(Ok(output)) => {
            let code = output.status.code();
            let mut combined = String::from_utf8_lossy(&output.stdout).into_owned();
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.trim().is_empty() {
                combined.push_str(&stderr);
            }
            let code_str = code.map_or_else(|| "signal".to_string(), |c| c.to_string());
            CommandOutcome {
                passed: output.status.success(),
                exit_code: code,
                output_tail: format!("$ {cmd}\n[exit {code_str}]\n{}", tail_chars(&combined)),
            }
        }
        Ok(Err(e)) => CommandOutcome {
            passed: false,
            exit_code: None,
            output_tail: format!("failed to run `{cmd}`: {e}"),
        },
        Err(_) => CommandOutcome {
            passed: false,
            exit_code: None,
            output_tail: format!("`{cmd}` timed out after {CHECK_TIMEOUT_SECS}s"),
        },
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
        let (passed, details, verified, command, exit_code) = match &request.command {
            Some(command) => {
                let outcome = run_check_command(command).await;
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
                )
            }
            None => (
                request.passed.unwrap_or(false),
                request.details.clone(),
                false,
                None,
                None,
            ),
        };
        let (report, results) = match &request.report {
            Some((format, path)) => {
                let (record, results) = crate::ship::report::read_report(format, path);
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
            report,
            results,
        }
    }
}
