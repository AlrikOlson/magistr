//! Carrying an engine command out on a port.

use magistr_engine::NativeFrame;

use super::event::EngineEvent;
use super::port::EnginePort;
use crate::cockpit::EngineCmd;

/// Carry `cmd` out on `port` and say what happened — the event the
/// consumer posts back to the cockpit as
/// [`Msg::Engine`](crate::cockpit::Msg::Engine). A refusal becomes
/// [`EngineEvent::PortFailed`] carrying the command and naming what
/// was asked. A checkpoint report is rendered to lines here, so the
/// cockpit never sees JSON.
///
/// A mutation's answer — the updated chunk or signal — comes back as
/// [`EngineEvent::Frame`] carrying a synthesized broadcast frame of the
/// kind the external server would have written, so the rail, the dock
/// and the Roadmap tab fold it through the one path they already have;
/// the in-process engines write no socket of their own.
pub fn apply(port: &mut dyn EnginePort, cmd: &EngineCmd) -> EngineEvent {
    match cmd {
        EngineCmd::Pin {
            step_number,
            pinned,
        } => match port.pin(*step_number, *pinned) {
            Ok(()) => EngineEvent::Pinned {
                step_number: *step_number,
                pinned: *pinned,
            },
            Err(error) => EngineEvent::PortFailed {
                cmd: cmd.clone(),
                what: format!("pin {step_number}"),
                error,
            },
        },
        EngineCmd::Checkpoint => match port.checkpoint() {
            Ok(report) => EngineEvent::Checkpoint {
                lines: lines(&report),
            },
            Err(error) => EngineEvent::PortFailed {
                cmd: cmd.clone(),
                what: "checkpoint".to_owned(),
                error,
            },
        },
        EngineCmd::StartChunk { id } => chunk_frame(
            port.start_chunk(id),
            "chunk_changed",
            format!("start {id}"),
            cmd,
        ),
        EngineCmd::CompleteChunk { id } => chunk_frame(
            port.complete_chunk(id),
            "chunk_completed",
            format!("complete {id}"),
            cmd,
        ),
        EngineCmd::ResolveProposal {
            chunk,
            kind,
            accept,
        } => chunk_frame(
            port.resolve_proposal(chunk, *kind, *accept),
            "chunk_changed",
            format!(
                "{} {} proposal on {chunk}",
                if *accept { "accept" } else { "reject" },
                kind.label()
            ),
            cmd,
        ),
        EngineCmd::ClearBlocker { chunk } => chunk_frame(
            port.clear_blocker(chunk),
            "chunk_changed",
            format!("clear the blocker on {chunk}"),
            cmd,
        ),
        EngineCmd::PromoteSignal { id } => signal_frame(
            port.promote_signal(id),
            "signal_changed",
            format!("promote {id}"),
            cmd,
        ),
        EngineCmd::IgnoreSignal { id } => signal_frame(
            port.ignore_signal(id),
            "signal_dismissed",
            format!("ignore {id}"),
            cmd,
        ),
        EngineCmd::SnoozeSignal { id, minutes } => signal_frame(
            port.snooze_signal(id, *minutes),
            "signal_changed",
            format!("snooze {id}"),
            cmd,
        ),
    }
}

/// A port's chunk answer as the broadcast frame the server would write.
fn chunk_frame(
    answer: Result<serde_json::Value, String>,
    kind: &str,
    what: String,
    cmd: &EngineCmd,
) -> EngineEvent {
    frame(answer, "roadmap", "chunk", kind, what, cmd)
}

/// A port's signal answer as the broadcast frame the server would write.
fn signal_frame(
    answer: Result<serde_json::Value, String>,
    kind: &str,
    what: String,
    cmd: &EngineCmd,
) -> EngineEvent {
    frame(answer, "signal", "signal", kind, what, cmd)
}

fn frame(
    answer: Result<serde_json::Value, String>,
    family: &str,
    key: &str,
    kind: &str,
    what: String,
    cmd: &EngineCmd,
) -> EngineEvent {
    match answer {
        Ok(payload) => EngineEvent::Frame(NativeFrame {
            family: family.to_owned(),
            kind: kind.to_owned(),
            payload: serde_json::json!({ "family": family, "type": kind, key: payload }),
        }),
        Err(error) => EngineEvent::PortFailed {
            cmd: cmd.clone(),
            what,
            error,
        },
    }
}

/// The report as readable lines: pretty-printed, since its shape is the
/// engine's to evolve.
fn lines(report: &serde_json::Value) -> Vec<String> {
    serde_json::to_string_pretty(report)
        .unwrap_or_else(|_| report.to_string())
        .lines()
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
#[path = "apply.tests.rs"]
mod tests;
