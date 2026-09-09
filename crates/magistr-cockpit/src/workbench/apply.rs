//! One workbench command, carried out on a port.

use super::port::WorkbenchPort;
use crate::cockpit::{WorkbenchCmd, WorkbenchEvent};

/// Carry `cmd` out on `port` and say what happened — the event the
/// consumer posts back to the cockpit as
/// [`Msg::Workbench`](crate::cockpit::Msg::Workbench). A refusal
/// becomes [`WorkbenchEvent::Failed`] naming what was asked.
pub fn apply(port: &mut dyn WorkbenchPort, cmd: &WorkbenchCmd) -> WorkbenchEvent {
    match cmd {
        WorkbenchCmd::Open { path } => match port.read(path) {
            Ok(content) => WorkbenchEvent::Opened {
                path: path.clone(),
                content,
            },
            Err(error) => failed(format!("open {path}"), error),
        },
        WorkbenchCmd::Undo { id } => match port.undo(id) {
            Ok(()) => WorkbenchEvent::Undone { id: id.clone() },
            Err(error) => failed(format!("undo {id}"), error),
        },
        WorkbenchCmd::Journal => match port.journal() {
            Ok(entries) => WorkbenchEvent::Journal { entries },
            Err(error) => failed("journal".to_owned(), error),
        },
        WorkbenchCmd::Search { query } => match port.search(query) {
            Ok(hits) => WorkbenchEvent::Found {
                query: query.clone(),
                hits,
            },
            Err(error) => failed(format!("search {query}"), error),
        },
        WorkbenchCmd::Map => match port.map() {
            Ok(lines) => WorkbenchEvent::Mapped { lines },
            Err(error) => failed("map".to_owned(), error),
        },
        WorkbenchCmd::Run { command } => match port.run(command) {
            Ok(rows) => WorkbenchEvent::Ran {
                command: command.clone(),
                rows,
            },
            Err(error) => failed(format!("run {command}"), error),
        },
    }
}

fn failed(what: String, error: String) -> WorkbenchEvent {
    WorkbenchEvent::Failed { what, error }
}

#[cfg(test)]
#[path = "apply.tests.rs"]
mod tests;
