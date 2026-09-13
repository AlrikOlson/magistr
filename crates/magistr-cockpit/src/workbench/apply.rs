//! One workbench command, carried out on a port.

use super::describe::describe;
use super::port::WorkbenchPort;
use crate::cockpit::{WorkbenchCmd, WorkbenchEvent};

/// Carry `cmd` out on `port` and say what happened — the event the
/// consumer posts back to the cockpit as
/// [`Msg::Workbench`](crate::cockpit::Msg::Workbench). A refusal
/// becomes [`WorkbenchEvent::Failed`] naming what was asked.
pub fn apply(port: &mut dyn WorkbenchPort, cmd: &WorkbenchCmd) -> WorkbenchEvent {
    let answer = match cmd {
        WorkbenchCmd::Open { path } => port.read(path).map(|content| WorkbenchEvent::Opened {
            path: path.clone(),
            content,
        }),
        WorkbenchCmd::Undo { id } => port
            .undo(id)
            .map(|()| WorkbenchEvent::Undone { id: id.clone() }),
        WorkbenchCmd::Journal => port
            .journal()
            .map(|entries| WorkbenchEvent::Journal { entries }),
        WorkbenchCmd::Search { query } => port.search(query).map(|hits| WorkbenchEvent::Found {
            query: query.clone(),
            hits,
        }),
        WorkbenchCmd::Map => port.map().map(|lines| WorkbenchEvent::Mapped { lines }),
        WorkbenchCmd::Run { command } => port.run(command).map(|rows| WorkbenchEvent::Ran {
            command: command.clone(),
            rows,
        }),
    };
    answer.unwrap_or_else(|error| WorkbenchEvent::Failed {
        what: describe(cmd),
        error,
    })
}

#[cfg(test)]
#[path = "apply.tests.rs"]
mod tests;
