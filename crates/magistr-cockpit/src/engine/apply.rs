//! Carrying an engine command out on a port.

use magistr_engine::native::{Chunk, Signal};
use magistr_engine::{NativeEvent, RoadmapFrame, SignalFrame};

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
/// A mutation's answer — the updated chunk or signal — comes back as a
/// local [`EngineEvent::Event`] carrying the typed frame the engines
/// would have announced for it, so the rail, the dock and the Roadmap
/// tab fold it through the one path they already have; the in-process
/// engines write no socket of their own.
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
        EngineCmd::StartChunk { id } => chunk_event(
            port.start_chunk(id),
            |chunk| RoadmapFrame::ChunkChanged { chunk },
            format!("start {id}"),
            cmd,
        ),
        EngineCmd::CompleteChunk { id } => chunk_event(
            port.complete_chunk(id),
            |chunk| RoadmapFrame::ChunkCompleted { chunk },
            format!("complete {id}"),
            cmd,
        ),
        EngineCmd::ResolveProposal {
            chunk,
            kind,
            accept,
        } => chunk_event(
            port.resolve_proposal(chunk, *kind, *accept),
            |chunk| RoadmapFrame::ChunkChanged { chunk },
            format!(
                "{} {} proposal on {chunk}",
                if *accept { "accept" } else { "reject" },
                kind.label()
            ),
            cmd,
        ),
        EngineCmd::ClearBlocker { chunk } => chunk_event(
            port.clear_blocker(chunk),
            |chunk| RoadmapFrame::ChunkChanged { chunk },
            format!("clear the blocker on {chunk}"),
            cmd,
        ),
        EngineCmd::PromoteSignal { id } => signal_event(
            port.promote_signal(id),
            |signal| SignalFrame::SignalChanged { signal },
            format!("promote {id}"),
            cmd,
        ),
        EngineCmd::IgnoreSignal { id } => signal_event(
            port.ignore_signal(id),
            |signal| SignalFrame::SignalDismissed { signal },
            format!("ignore {id}"),
            cmd,
        ),
        EngineCmd::SnoozeSignal { id, minutes } => signal_event(
            port.snooze_signal(id, *minutes),
            |signal| SignalFrame::SignalChanged { signal },
            format!("snooze {id}"),
            cmd,
        ),
    }
}

/// A port's chunk answer as the roadmap event the engines would announce.
fn chunk_event(
    answer: Result<Chunk, String>,
    frame: impl FnOnce(Chunk) -> RoadmapFrame,
    what: String,
    cmd: &EngineCmd,
) -> EngineEvent {
    match answer {
        Ok(chunk) => EngineEvent::local(NativeEvent::Roadmap(frame(chunk))),
        Err(error) => EngineEvent::PortFailed {
            cmd: cmd.clone(),
            what,
            error,
        },
    }
}

/// A port's signal answer as the signal event the engines would announce.
fn signal_event(
    answer: Result<Signal, String>,
    frame: impl FnOnce(Signal) -> SignalFrame,
    what: String,
    cmd: &EngineCmd,
) -> EngineEvent {
    match answer {
        Ok(signal) => EngineEvent::local(NativeEvent::Signal(frame(signal))),
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
