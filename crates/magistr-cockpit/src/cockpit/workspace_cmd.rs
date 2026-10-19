//! What the cockpit asks of the workspace.

/// A workspace request from the cockpit, carried by
/// [`Cmd::Workspace`](super::Cmd::Workspace): the consumer runs it
/// through a [`WorkspacePort`](crate::workspace::WorkspacePort) and
/// posts the outcome back as a
/// [`WorkspaceEvent`](super::WorkspaceEvent).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum WorkspaceCmd {
    /// \[Open\] (or `/read`): read the file as it stands.
    Open {
        /// The file the card or the command names.
        path: String,
    },
    /// \[Undo\]: rewind the journal step behind the edit.
    Undo {
        /// The edit's id, as the journal carries it.
        id: String,
    },
    /// Refresh the Changes tab from the write journal.
    Journal,
    /// `/search`: ranked hits without the agent.
    Search {
        /// What to look for.
        query: String,
    },
    /// `/map`: the workspace map without the agent.
    Map,
    /// `/run`: one declared command, rows back.
    Run {
        /// The declared command's name.
        command: String,
    },
}
