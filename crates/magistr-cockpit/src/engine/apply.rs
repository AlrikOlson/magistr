//! Carrying an engine command out on a port.

use magistr_workflow_contracts::{Chunk, Observed, Signal, WorkflowMutationError};
use magistr_workflow_contracts::{NativeEvent, RoadmapFrame, SignalFrame};

use super::describe::describe;
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
    let answer = match cmd {
        EngineCmd::Pin {
            step_number,
            pinned,
        } => port
            .pin(*step_number, *pinned)
            .map(|_| EngineEvent::Pinned {
                step_number: *step_number,
                pinned: *pinned,
            }),
        EngineCmd::Checkpoint => port
            .checkpoint()
            .map_err(WorkflowMutationError::rejected)
            .map(|report| EngineEvent::Checkpoint {
                lines: lines(&report),
            }),
        EngineCmd::StartChunk { id } => chunk_event(port.start_chunk(id), |chunk| {
            RoadmapFrame::ChunkChanged { chunk }
        }),
        EngineCmd::CompleteChunk { id } => chunk_event(port.complete_chunk(id), |chunk| {
            RoadmapFrame::ChunkCompleted { chunk }
        }),
        EngineCmd::ResolveProposal {
            chunk,
            kind,
            accept,
        } => chunk_event(port.resolve_proposal(chunk, *kind, *accept), |chunk| {
            RoadmapFrame::ChunkChanged { chunk }
        }),
        EngineCmd::ClearBlocker { chunk } => chunk_event(port.clear_blocker(chunk), |chunk| {
            RoadmapFrame::ChunkChanged { chunk }
        }),
        EngineCmd::PromoteSignal { id } => signal_event(
            port.promote_signal(id).map(|promotion| promotion.signal),
            |signal| SignalFrame::SignalChanged { signal },
        ),
        EngineCmd::IgnoreSignal { id } => signal_event(port.ignore_signal(id), |signal| {
            SignalFrame::SignalDismissed { signal }
        }),
        EngineCmd::SnoozeSignal { id, minutes } => {
            signal_event(port.snooze_signal(id, *minutes), |signal| {
                SignalFrame::SignalChanged { signal }
            })
        }
    };
    answer.unwrap_or_else(|error| EngineEvent::PortFailed {
        cmd: cmd.clone(),
        what: describe(cmd),
        error: error.to_string(),
    })
}

/// A port's chunk answer as the roadmap event the engines would announce.
fn chunk_event(
    answer: Result<Observed<Chunk>, WorkflowMutationError>,
    frame: impl FnOnce(Chunk) -> RoadmapFrame,
) -> Result<EngineEvent, WorkflowMutationError> {
    answer.map(|chunk| EngineEvent::local(NativeEvent::Roadmap(frame(chunk.value))))
}

/// A port's signal answer as the signal event the engines would announce.
fn signal_event(
    answer: Result<Observed<Signal>, WorkflowMutationError>,
    frame: impl FnOnce(Signal) -> SignalFrame,
) -> Result<EngineEvent, WorkflowMutationError> {
    answer.map(|signal| EngineEvent::local(NativeEvent::Signal(frame(signal.value))))
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
