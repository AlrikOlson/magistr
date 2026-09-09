//! How a row is styled.

use super::voice::Voice;
use crate::agent::ToolStatus;
use crate::layout::TextRole;

/// The block-level kind of one wrapped row — what the pen styles.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RowKind {
    /// Plain text in a voice.
    Prose(Voice),
    /// A markdown heading line (`# …`), its marker stripped.
    Heading,
    /// A line of a fenced code block.
    Code,
    /// A fence marker line (`` ``` ``): the block's frame, dimmed.
    Fence,
    /// A list item (`- `, `* `, `+ ` or `1. `), continuation rows indented.
    Bullet,
    /// A tool call, by status.
    Tool(ToolStatus),
    /// A quality-gate check's head row, by outcome.
    Check(bool),
    /// An added line of an edit card's diff.
    DiffAdd,
    /// A removed line of an edit card's diff.
    DiffRemove,
    /// A merge-conflict marker line — the one kind of diff row the
    /// human has to resolve before anything else can happen.
    Conflict,
    /// An expanded edit card's button row — the view carves it into
    /// hittable [Open] and [Undo] segments.
    EditButtons,
    /// One row of an image block: the view fills it — with a pixel
    /// surface when the terminal composites graphics, with halfblocks
    /// otherwise — and the pen only supplies its ground.
    Image,
    /// One row of an open card's terminal pane: the view fills it from
    /// the program's own screen, cell by cell, and the pen only
    /// supplies the pane's ground.
    Pane,
    /// A think-and-ship event row.
    Event,
    /// A note from the cockpit.
    Note,
    /// An error.
    Error,
}

impl RowKind {
    /// Which voice this row is set in on a renderer that has two.
    ///
    /// The kind is already the classification the question needs, so
    /// there is one decision here rather than a guess at each call site
    /// — the same reason `turn::diff_kind` is one classifier the Changes
    /// tab and the turn feed both read. Read only by the native
    /// painter; a cell grid has one voice.
    ///
    /// Everything that is *aligned* stays mono, and the test is not
    /// "is it code" but "does a column mean something here": a diff, a
    /// fence, a tool card's status row, a gate's outcome, an edit card's
    /// button row and an image's halfblocks are all laid out by counting
    /// columns. So is a [`RowKind::Bullet`] — its marker and the wrapped
    /// continuation under it are lined up with spaces, and a
    /// proportional space is a third of a mono one, so setting a list
    /// proportionally would step every continuation row left.
    #[must_use]
    pub const fn text_role(self) -> TextRole {
        match self {
            Self::Prose(_) | Self::Note | Self::Error => TextRole::Prose,
            Self::Heading => TextRole::Heading,
            Self::Bullet
            | Self::Code
            | Self::Fence
            | Self::Tool(_)
            | Self::Check(_)
            | Self::DiffAdd
            | Self::DiffRemove
            | Self::Conflict
            | Self::EditButtons
            | Self::Image
            | Self::Pane
            | Self::Event => TextRole::Data,
        }
    }
}

#[cfg(test)]
#[path = "row_kind.tests.rs"]
mod tests;
