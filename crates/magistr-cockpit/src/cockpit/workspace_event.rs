//! What the workspace answered.

use super::workspace_cmd::WorkspaceCmd;
use crate::workspace::{JournalEntry, RunRow};

/// The outcome of a [`WorkspaceCmd`](super::WorkspaceCmd), posted on
/// the inbox as [`Msg::Workspace`](super::Msg::Workspace) and folded
/// into the model — the Changes tab is where most of it lands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkspaceEvent {
    /// The file was read: its content opens in the Changes tab's
    /// read-only viewer pane.
    Opened {
        /// The file that was read.
        path: String,
        /// Its content as it stands.
        content: String,
    },
    /// The journal rewound the edit: it leaves the Changes list and its
    /// turn card reads ` · undone`.
    Undone {
        /// The edit's id.
        id: String,
    },
    /// The write journal as it stands — the Changes tab's list.
    Journal {
        /// Every journaled edit, oldest first.
        entries: Vec<JournalEntry>,
    },
    /// A search answered: the hits open in the viewer pane.
    Found {
        /// What was looked for.
        query: String,
        /// The hits, one line each.
        hits: Vec<String>,
    },
    /// The map answered: its lines open in the viewer pane.
    Mapped {
        /// The map, one line per entry.
        lines: Vec<String>,
    },
    /// A run finished: its rows land on the Changes tab, failing rows
    /// first.
    Ran {
        /// The declared command that ran.
        command: String,
        /// The structured rows, in the runner's order.
        rows: Vec<RunRow>,
    },
    /// The workspace refused: an error line on the stage, and — for a
    /// refused undo — a refusal on the journal entry it named, so a
    /// conflict (a later change touched the file) is shown where the
    /// change is.
    Failed {
        /// What was asked, as the command itself, so the fold can land
        /// the refusal on the object the ask was about.
        cmd: WorkspaceCmd,
        /// The workspace's reason.
        error: String,
    },
}
