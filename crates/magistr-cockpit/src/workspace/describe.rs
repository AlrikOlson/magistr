//! What a workspace command asked, in the words a refusal names.

use crate::cockpit::WorkspaceCmd;

/// The ask behind `cmd` — `open <path>`, `undo <id>`, `journal`,
/// `search <q>`, `map`, `run <command>` — the words a refusal
/// ([`WorkspaceEvent::Failed`](crate::cockpit::WorkspaceEvent::Failed))
/// is reported on the stage with, and the name a session job runs under.
#[must_use]
pub fn describe(cmd: &WorkspaceCmd) -> String {
    match cmd {
        WorkspaceCmd::Open { path } => format!("open {path}"),
        WorkspaceCmd::Undo { id } => format!("undo {id}"),
        WorkspaceCmd::Journal => "journal".to_owned(),
        WorkspaceCmd::Search { query } => format!("search {query}"),
        WorkspaceCmd::Map => "map".to_owned(),
        WorkspaceCmd::Run { command } => format!("run {command}"),
    }
}
