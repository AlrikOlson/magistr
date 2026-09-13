//! What a workbench command asked, in the words a refusal names.

use crate::cockpit::WorkbenchCmd;

/// The ask behind `cmd` — `open <path>`, `undo <id>`, `journal`,
/// `search <q>`, `map`, `run <command>` — as
/// [`WorkbenchEvent::Failed`](crate::cockpit::WorkbenchEvent::Failed)
/// carries it.
#[must_use]
pub fn describe(cmd: &WorkbenchCmd) -> String {
    match cmd {
        WorkbenchCmd::Open { path } => format!("open {path}"),
        WorkbenchCmd::Undo { id } => format!("undo {id}"),
        WorkbenchCmd::Journal => "journal".to_owned(),
        WorkbenchCmd::Search { query } => format!("search {query}"),
        WorkbenchCmd::Map => "map".to_owned(),
        WorkbenchCmd::Run { command } => format!("run {command}"),
    }
}
