//! Which role's accent bar a turn line carries.

use super::line_kind::LineKind;
use super::voice::Voice;

/// The role a line of the turn speaks for, as its gutter bar shows it:
/// the user's prompt, the agent's message, the agent's thinking, a tool
/// at work — or none, for the cockpit's own meta rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Accent {
    /// The user's echoed prompt.
    User,
    /// The agent's message.
    Agent,
    /// The agent's thinking.
    Thought,
    /// A tool call, check or edit card — and its expanded detail.
    Tool,
    /// A meta row (event, note, error): no bar.
    Meta,
}

impl Accent {
    /// Columns the gutter takes ahead of every turn row: the bar and a
    /// gap.
    pub const GUTTER_COLS: u16 = 2;

    /// The accent of a line of `kind`.
    #[must_use]
    pub const fn of(kind: &LineKind) -> Self {
        match kind {
            LineKind::Prose(Voice::User) => Self::User,
            LineKind::Prose(Voice::Agent) | LineKind::Image(_) => Self::Agent,
            LineKind::Prose(Voice::Thought) => Self::Thought,
            LineKind::Tool { .. } | LineKind::Check { .. } | LineKind::Edit { .. } => Self::Tool,
            LineKind::Event | LineKind::Note | LineKind::Error => Self::Meta,
        }
    }

    /// The gutter's text: the bar glyph and its gap, always
    /// [`Self::GUTTER_COLS`] columns; a meta row's is blank.
    #[must_use]
    pub const fn gutter(self) -> &'static str {
        match self {
            Self::Meta => "  ",
            _ => "▎ ",
        }
    }
}

#[cfg(test)]
#[path = "accent.tests.rs"]
mod tests;
