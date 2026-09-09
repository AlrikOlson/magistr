//! How an inline run is styled.

use crate::render::Color;

/// The inline style of one span of a row's text — what the pen lays over
/// the row's own style.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SpanStyle {
    /// `**strong**` or `__strong__`.
    Bold,
    /// `*emphasis*` or `_emphasis_`.
    Italic,
    /// `` `code` ``: the code pen, inline.
    Code,
    /// `[label](url)`: the label, underlined, carrying the url as an
    /// OSC 8 hyperlink.
    Link(String),
    /// A syntax-highlighted run of a fenced code line: the theme's
    /// foreground over the code pen.
    Fg(Color),
}
