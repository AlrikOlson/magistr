//! A check as a job on the blocking pool.

use std::path::PathBuf;

use magistr_cockpit::cockpit::Msg;
use magistr_cockpit::engine::EngineEvent;
use magistr_cockpit::job::JobOutput;
use magistr_cockpit_runtime::channel::Inbox;
use magistr_engine::{CheckBytes, MagistrFrame, NativeEvent};
use tokio::runtime::Handle;
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;

use crate::utils::check::CheckRun;
use crate::utils::check::run_check;

/// The closure [`Jobs::spawn_job`](magistr_cockpit_runtime::Jobs::spawn_job)
/// runs for the check `name`: `command` under `sh -c` in `dir` through the
/// shared check runner — the same one `magistr_check` goes through, so the
/// two paths cannot disagree about what a check did — driven on the
/// runtime's handle from the blocking thread. The check runs on a pty, and
/// every task it writes goes to `inbox` while it runs as a
/// [`CheckBytes`] event keyed by `name` — the same event `magistr
/// mcp`'s socket carries for the agent's own checks, folded into the
/// cockpit's terminal store either way. The whole run goes to the driver on `report`: the stripped
/// transcript, the parsed failure rows, the real exit code, and a verdict
/// that tells a timeout apart from a red test. The job's own result — pass
/// as text, fail as a failure — reaches the cockpit as `Msg::Job` and
/// settles the check's LED. `cancel` is the driver's way out mid-check: when
/// it fires (the cockpit closed), the check's process group is ended and
/// the job returns promptly instead of keeping the runtime's shutdown
/// waiting on the child.
pub(crate) fn check_job(
    name: String,
    command: String,
    dir: PathBuf,
    report: UnboundedSender<CheckRun>,
    inbox: Inbox,
    cancel: CancellationToken,
) -> impl FnOnce() -> JobOutput + Send + 'static {
    move || {
        let check = name.clone();
        let stream = move |bytes: &[u8]| {
            let output = CheckBytes {
                check: check.clone(),
                bytes: bytes.to_vec(),
            };
            inbox.send(Msg::Engine(EngineEvent::local(NativeEvent::Magistr(
                MagistrFrame::CheckBytes(output),
            ))));
        };
        // Every check the loop runs is one the project requires: the set is
        // the required contracts completion verifies, never an optional
        // custom check.
        let run = Handle::current().block_on(run_check(
            &name, &command, true, &dir,
            0, // Resolve the same configured deadline as MCP and native checks.
            &cancel, stream,
        ));
        let (line, passed) = (run.line(), run.passed());
        let _ = report.send(run);
        if passed {
            JobOutput::Text(line)
        } else {
            JobOutput::Failed(line)
        }
    }
}

#[cfg(all(test, unix))]
#[path = "check_job.tests.rs"]
mod tests;
