//! One line of the flat cockpit.

use super::role::Role;

/// A line of the flat rendering: who it is from, what it says, and
/// whether it can still change.
///
/// `role` is `None` for a **continuation** — a further line of something
/// already introduced (a gate's details, an edit's diff body, a blank
/// line inside prose). Repeating the label on every line of a forty-line
/// diff would be forty words a reader has to hear past; a two-space
/// indent says "still the thing above" once.
///
/// `settled` is the whole redraw-free contract: a line is settled when
/// nothing can change it any more, and only settled lines are ever
/// written out. See [`lines`](super::lines) for the rule per kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlatLine {
    role: Option<Role>,
    text: String,
    settled: bool,
}

impl FlatLine {
    /// A line of `role` saying `text`, settled or not.
    #[must_use]
    pub fn new(role: Option<Role>, text: impl Into<String>, settled: bool) -> Self {
        Self {
            role,
            text: text.into(),
            settled,
        }
    }

    /// A line nothing can change any more.
    #[must_use]
    pub fn settled(role: Option<Role>, text: impl Into<String>) -> Self {
        Self::new(role, text, true)
    }

    /// Who the line is from; `None` for a continuation.
    #[must_use]
    pub const fn role(&self) -> Option<Role> {
        self.role
    }

    /// What the line says, without its label.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Whether nothing can change this line any more.
    #[must_use]
    pub const fn is_settled(&self) -> bool {
        self.settled
    }

    /// The line as it goes to the terminal: `role: text`, or two spaces
    /// and the text for a continuation. Trailing space is trimmed, so a
    /// blank continuation is a blank line rather than two spaces a
    /// reader would announce.
    #[must_use]
    pub fn render(&self) -> String {
        let rendered = match self.role {
            Some(role) => format!("{}: {}", role.label(), self.text),
            None => format!("  {}", self.text),
        };
        rendered.trim_end().to_owned()
    }
}

#[cfg(test)]
#[path = "flat_line.tests.rs"]
mod tests;
