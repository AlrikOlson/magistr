//! A quality gate as a job on the blocking pool.

use std::path::PathBuf;

use magistr_cockpit::channel::Inbox;
use magistr_cockpit::cockpit::Msg;
use magistr_cockpit::engine::EngineEvent;
use magistr_cockpit::job::JobOutput;
use magistr_engine::{GateOutput, MagistrFrame, NativeEvent};
use tokio::runtime::Handle;
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;

use super::gate_outcome::GateOutcome;
use crate::utils::gate::run_gate;

/// How long one gate may run, as the legacy gate runner allowed.
const GATE_TIMEOUT_SECS: u64 = 300;

/// The closure [`Jobs::spawn_job`](magistr_cockpit::job::Jobs::spawn_job)
/// runs for the gate `name`: `command` under `sh -c` in `dir` through the
/// shared gate runner — the same one `magistr_gate` goes through, so the
/// two paths cannot disagree about what a gate did — driven on the
/// runtime's handle from the blocking thread. The gate runs on a pty, and
/// every chunk it writes goes to `inbox` while it runs as a
/// [`GateOutput`] event keyed by `name` — the same event `magistr
/// mcp-serve`'s socket carries for the agent's own gates, folded into the
/// cockpit's terminal store either way. The whole run goes to the driver on `report`: the stripped
/// transcript, the parsed failure rows, the real exit code, and a verdict
/// that tells a timeout apart from a red test. The job's own result — pass
/// as text, fail as a failure — reaches the cockpit as `Msg::Job` and
/// settles the gate's LED. `cancel` is the driver's way out mid-gate: when
/// it fires (the cockpit closed), the gate's process group is ended and
/// the job returns promptly instead of keeping the runtime's shutdown
/// waiting on the child.
pub(crate) fn gate_job(
    name: &'static str,
    command: String,
    dir: PathBuf,
    report: UnboundedSender<GateOutcome>,
    inbox: Inbox,
    cancel: CancellationToken,
) -> impl FnOnce() -> JobOutput + Send + 'static {
    move || {
        let stream = move |bytes: &[u8]| {
            let output = GateOutput {
                gate: name.to_owned(),
                bytes: bytes.to_vec(),
            };
            inbox.send(Msg::Engine(EngineEvent::local(NativeEvent::Magistr(
                MagistrFrame::GateOutput(output),
            ))));
        };
        // Every gate the loop runs is one the project requires: they are
        // the top-level `quality_gate`, not the optional custom set.
        let run = Handle::current().block_on(run_gate(
            name,
            &command,
            true,
            &dir,
            GATE_TIMEOUT_SECS,
            &cancel,
            stream,
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

#[cfg(test)]
#[path = "gate_job.tests.rs"]
mod tests;
