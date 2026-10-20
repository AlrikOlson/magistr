//! Run a shell command under a pseudo-terminal and stream what it writes.
//!
//! [`run_command_live`] is the runner behind every quality gate. The
//! command sees a tty — colours, progress bars, its own width — every
//! chunk it writes reaches the caller's sink raw and in order while it
//! runs, and the result carries the escape-stripped transcript the trace
//! keeps. Where no pty can be had (a platform without one, a master that
//! will not open) the same owned process runs on pipes and the sink gets
//! each chunk as it lands: a text-only pane rather than a blank one. The
//! process mechanics — the group, the cap, the cancel, the graceful end —
//! are [`magistr_process`]'s; what this module decides is that a gate is
//! a configured shell line on a terminal.

use std::path::Path;

use magistr_process::{EscapeStripper, OwnedProcess, ProcessOutcome};
use tokio_util::sync::CancellationToken;

use super::subprocess::{
    CommandResult, command_result, run_guarded, shell_spec, skipped, spawn_failed,
};

/// The PTY's fixed size. Its 140 columns match rustc's default diagnostic
/// width outside a terminal, keeping recorded diagnostics consistently wrapped.
const PTY_SIZE: (u16, u16) = (140, 40);

/// Run `sh -c command` in `dir` on a pty, streaming every chunk it writes
/// to `sink` as it lands and answering with the stripped transcript in
/// `stdout` (`stderr` names only how the run ended: a tty is one stream).
/// The timeout, the cancel and the process-group cleanup are the shared
/// mechanism's, and so are the results they answer with.
pub async fn run_command_live(
    command: &str,
    dir: &Path,
    timeout_secs: u64,
    cancel: &CancellationToken,
    mut sink: impl FnMut(&[u8]) + Send,
) -> CommandResult {
    if command.is_empty() {
        let result = skipped();
        sink(result.stdout.as_bytes());
        return result;
    }
    let spec = shell_spec(command, dir, timeout_secs).on_terminal(PTY_SIZE.0, PTY_SIZE.1);
    let process = match OwnedProcess::spawn(spec) {
        Ok(process) => process,
        Err(error) => return spawn_failed(&error),
    };
    if !process.is_terminal() {
        tracing::debug!(
            target: "magistr::gate",
            "no pty for the command, running it on pipes"
        );
        let outcome = run_guarded(process, cancel, &mut sink).await;
        return command_result(outcome, timeout_secs);
    }
    let mut stripper = EscapeStripper::default();
    let mut text = String::new();
    let outcome = run_guarded(process, cancel, |chunk| {
        sink(chunk);
        text.push_str(&stripper.feed(chunk));
    })
    .await;
    let mut result = command_result(
        ProcessOutcome {
            stdout: String::new(),
            ..outcome
        },
        timeout_secs,
    );
    result.stdout = text;
    result
}

#[cfg(test)]
#[path = "live_command.tests.rs"]
mod tests;
