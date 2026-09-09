//! How a cell is painted: colors, attributes and an optional hyperlink.

use std::sync::Arc;

use super::attrs::Attrs;
use super::color::Color;

/// The visual style of a cell.
///
/// `link` is an OSC 8 hyperlink target; cells sharing the same `Arc` share
/// one link run in the encoded stream. `underline_color` is the rule's
/// own colour (SGR 58), independent of `fg` — what lets a curly rule
/// read as an error under text that stays legible.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Style {
    /// Foreground color.
    pub fg: Color,
    /// Background color.
    pub bg: Color,
    /// Text attributes.
    pub attrs: Attrs,
    /// The colour of the underline rule (SGR 58). [`Color::Default`] is
    /// the terminal's own answer — the text colour on every terminal
    /// that draws one — so a cell that never asked for a separate rule
    /// colour encodes exactly as it always did.
    pub underline_color: Color,
    /// OSC 8 hyperlink target, if any.
    pub link: Option<Arc<str>>,
}

impl Style {
    /// The terminal default: default colors, no attributes, no link.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the foreground color.
    #[must_use]
    pub fn fg(mut self, fg: Color) -> Self {
        self.fg = fg;
        self
    }

    /// Set the background color.
    #[must_use]
    pub fn bg(mut self, bg: Color) -> Self {
        self.bg = bg;
        self
    }

    /// Set the attributes.
    #[must_use]
    pub fn attrs(mut self, attrs: Attrs) -> Self {
        self.attrs = attrs;
        self
    }

    /// Set the underline rule's colour (SGR 58).
    #[must_use]
    pub fn underline_color(mut self, color: Color) -> Self {
        self.underline_color = color;
        self
    }

    /// Set the hyperlink target.
    #[must_use]
    pub fn link(mut self, url: impl Into<Arc<str>>) -> Self {
        self.link = Some(url.into());
        self
    }

    /// True when the terminal's SGR state after `\x1b[0m` matches this
    /// style (the link is a separate OSC 8 state and is not considered).
    ///
    /// The underline colour counts: `SGR 0` resets it along with
    /// everything else, so a pen carrying one is not the default pen and
    /// the diff must say so.
    #[must_use]
    pub fn is_default_sgr(&self) -> bool {
        self.fg == Color::Default
            && self.bg == Color::Default
            && self.attrs.is_empty()
            && self.underline_color == Color::Default
    }
}
