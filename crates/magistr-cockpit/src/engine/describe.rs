//! What an engine command asked, in the words a refusal names.

use crate::cockpit::EngineCmd;

/// The ask behind `cmd` — `pin 7`, `checkpoint`, `start <id>`,
/// `promote <id>`, … — as
/// [`EngineEvent::PortFailed`](super::EngineEvent::PortFailed) carries it.
#[must_use]
pub fn describe(cmd: &EngineCmd) -> String {
    match cmd {
        EngineCmd::Pin { step_number, .. } => format!("pin {step_number}"),
        EngineCmd::Checkpoint => "checkpoint".to_owned(),
        EngineCmd::StartTask { id } => format!("start {id}"),
        EngineCmd::CompleteTask { id } => format!("complete {id}"),
        EngineCmd::ResolveProposal { task, kind, accept } => format!(
            "{} {} proposal on {task}",
            if *accept { "accept" } else { "reject" },
            kind.label()
        ),
        EngineCmd::ClearBlocker { task } => format!("clear the blocker on {task}"),
        EngineCmd::PromoteSignal { id } => format!("promote {id}"),
        EngineCmd::IgnoreSignal { id } => format!("ignore {id}"),
        EngineCmd::SnoozeSignal { id, .. } => format!("snooze {id}"),
    }
}
