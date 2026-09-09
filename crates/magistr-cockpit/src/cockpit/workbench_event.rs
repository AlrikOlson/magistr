//! What the workbench answered.

use crate::workbench::{JournalEntry, RunRow};

/// The outcome of a [`WorkbenchCmd`](super::WorkbenchCmd), posted on
/// the inbox as [`Msg::Workbench`](super::Msg::Workbench) and folded
/// into the model — the Changes tab is where most of it lands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkbenchEvent {
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
    /// The workbench refused: an error line on the stage.
    Failed {
        /// What was asked (`open <path>`, `undo <id>`, `search <q>`, …).
        what: String,
        /// The workbench's reason.
        error: String,
    },
}
