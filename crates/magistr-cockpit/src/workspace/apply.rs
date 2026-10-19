//! One workspace command, carried out on a port.

use crate::cockpit::{WorkspaceCmd, WorkspaceEvent};
use crate::workspace::port::WorkspacePort;

/// Carry `cmd` out on `port` and say what happened — the event the
/// consumer posts back to the cockpit as
/// [`Msg::Workspace`](crate::cockpit::Msg::Workspace). A refusal
/// becomes [`crate::cockpit::WorkspaceEvent::Failed`] carrying what was asked.
pub fn apply(port: &mut dyn WorkspacePort, cmd: &WorkspaceCmd) -> WorkspaceEvent {
    let answer = match cmd {
        crate::cockpit::WorkspaceCmd::Open { path } => {
            port.read(path)
                .map(|content| crate::cockpit::WorkspaceEvent::Opened {
                    path: path.clone(),
                    content,
                })
        }
        crate::cockpit::WorkspaceCmd::Undo { id } => port
            .undo(id)
            .map(|()| crate::cockpit::WorkspaceEvent::Undone { id: id.clone() }),
        crate::cockpit::WorkspaceCmd::Journal => port
            .journal()
            .map(|entries| crate::cockpit::WorkspaceEvent::Journal { entries }),
        crate::cockpit::WorkspaceCmd::Search { query } => {
            port.search(query)
                .map(|hits| crate::cockpit::WorkspaceEvent::Found {
                    query: query.clone(),
                    hits,
                })
        }
        crate::cockpit::WorkspaceCmd::Map => port
            .map()
            .map(|lines| crate::cockpit::WorkspaceEvent::Mapped { lines }),
        crate::cockpit::WorkspaceCmd::Run { command } => {
            port.run(command)
                .map(|rows| crate::cockpit::WorkspaceEvent::Ran {
                    command: command.clone(),
                    rows,
                })
        }
    };
    answer.unwrap_or_else(|error| crate::cockpit::WorkspaceEvent::Failed {
        cmd: cmd.clone(),
        error,
    })
}

#[cfg(test)]
#[path = "apply.tests.rs"]
mod tests;
