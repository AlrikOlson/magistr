//! Carrying an engine command out on a port.

use magistr_workflow_contracts::{NativeEvent, PlanFrame, SignalFrame};
use magistr_workflow_contracts::{Observed, Signal, Task, WorkflowMutationError};

use super::describe::describe;
use super::event::EngineEvent;
use super::port::EnginePort;
use crate::cockpit::EngineCmd;

/// Carry `cmd` out on `port` and say what happened — the event the
/// consumer posts back to the cockpit as
/// [`Msg::Engine`](crate::cockpit::Msg::Engine). A refusal becomes
/// [`EngineEvent::PortFailed`] carrying the command and naming what
/// was asked. A review report is rendered to lines here, so the
/// cockpit never sees JSON.
///
/// A mutation's answer — the updated task or signal — comes back as a
/// local [`EngineEvent::Event`] carrying the typed frame the engines
/// would have announced for it, so the rail, the dock and the Plan
/// tab fold it through the one path they already have; the in-process
/// engines write no socket of their own.
pub fn apply(port: &mut dyn EnginePort, cmd: &EngineCmd) -> EngineEvent {
    let answer = match cmd {
        EngineCmd::Pin { note, pinned } => port.pin(*note, *pinned).map(|_| EngineEvent::Pinned {
            note: *note,
            pinned: *pinned,
        }),
        EngineCmd::Review => port
            .review()
            .map_err(WorkflowMutationError::rejected)
            .map(|report| EngineEvent::Review {
                lines: lines(&report),
            }),
        EngineCmd::StartTask { id } => {
            task_event(port.start_task(id), |task| PlanFrame::TaskChanged { task })
        }
        EngineCmd::CompleteTask { id } => task_event(port.complete_task(id), |task| {
            PlanFrame::TaskCompleted { task }
        }),
        EngineCmd::ResolveProposal { task, kind, accept } => {
            task_event(port.resolve_proposal(task, *kind, *accept), |task| {
                PlanFrame::TaskChanged { task }
            })
        }
        EngineCmd::ClearBlocker { task } => task_event(port.clear_blocker(task), |task| {
            PlanFrame::TaskChanged { task }
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

/// A port's task answer as the plan event the engines would announce.
fn task_event(
    answer: Result<Observed<Task>, WorkflowMutationError>,
    frame: impl FnOnce(Task) -> PlanFrame,
) -> Result<EngineEvent, WorkflowMutationError> {
    answer.map(|task| EngineEvent::local(NativeEvent::Plan(frame(task.value))))
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
