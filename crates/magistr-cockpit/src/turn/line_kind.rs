//! What a source line is.

use super::image::StreamImage;
use super::voice::Voice;
use crate::agent::{ToolStatus, ToolVerb};

/// The kind of one source line of a turn.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum LineKind {
    /// Streamed text in a [`Voice`]; the last one may still be open.
    Prose(Voice),
    /// One tool call — collapsed to one row (glyph, verb, cut title),
    /// updated in place by id, expanded to its full title on demand.
    Tool {
        /// The call's id.
        id: String,
        /// Where the call stands.
        status: ToolStatus,
        /// What kind of thing it does — the verb column.
        verb: ToolVerb,
        /// How long it ran, once the session measured its finish.
        duration_ms: Option<u64>,
        /// Whether the full title shows below the collapsed row.
        expanded: bool,
    },
    /// One quality-gate check recorded on the ship trace — collapsed to
    /// one row (glyph, name, exit code, duration); the line's text
    /// carries the details shown expanded.
    Check {
        /// The gate's name (`cargo test`, `lint`, …).
        name: String,
        /// Whether it passed.
        passed: bool,
        /// The process exit code, when the check ran a command.
        exit_code: Option<i64>,
        /// How long it ran, when measured.
        duration_ms: Option<u64>,
        /// Whether the details show below the collapsed row.
        expanded: bool,
    },
    /// One workbench edit — collapsed to one row (status glyph, `edit`,
    /// the path, added/removed counts); the line's text carries the
    /// unified diff body shown expanded, under an [Open]/[Undo] button
    /// row.
    Edit {
        /// The tool call's id.
        id: String,
        /// The file the edit landed on.
        path: String,
        /// Where the call stands.
        status: ToolStatus,
        /// Whether the buttons and the diff show below the collapsed row.
        expanded: bool,
        /// Whether the workbench journal rewound this edit.
        undone: bool,
    },
    /// One image the agent sent, decoded — as many rows as its
    /// footprint at the stage width takes. The line's text is the alt
    /// text, kept for a reader that cannot see the pixels; an image
    /// that would not decode never becomes this line, it becomes a
    /// [`LineKind::Note`] carrying the alt text.
    Image(StreamImage),
    /// One think-and-ship event — a task started, a step recorded — as
    /// one dim row, its glyph baked into the text.
    Event,
    /// Something the cockpit noted (a turn that ended early).
    Note,
    /// Something that failed (the transport, the protocol).
    Error,
}
