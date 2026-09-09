//! Turn a frame's changes into the fewest bytes the terminal needs.
//!
//! Every frame is bracketed by DEC 2026 synchronized output (BSU
//! `CSI ? 2026 h`, ESU `CSI ? 2026 l`) so the terminal paints it
//! atomically. Inside the bracket the encoder moves the cursor only when a
//! change is not adjacent to the previous one, emits an SGR only when the
//! pen changes, opens and closes OSC 8 hyperlinks on link boundaries, and
//! leaves the terminal at the default pen so the next frame starts from a
//! known state.

use std::sync::Arc;

use super::change::Change;
use magistr_cockpit::render::Attrs;
use magistr_cockpit::render::Color;
use magistr_cockpit::render::Headline;
use magistr_cockpit::render::Style;
use magistr_cockpit::render::Underline;

/// Begin synchronized update.
const BSU: &[u8] = b"\x1b[?2026h";
/// End synchronized update.
const ESU: &[u8] = b"\x1b[?2026l";
/// Erase the whole display.
const CLEAR: &[u8] = b"\x1b[2J";
/// Reset every SGR attribute and color.
const SGR_RESET: &[u8] = b"\x1b[0m";
/// Close the current OSC 8 hyperlink.
const LINK_CLOSE: &[u8] = b"\x1b]8;;\x1b\\";

/// Encodes frame diffs into terminal bytes, remembering the cursor and pen
/// between frames so unchanged state costs nothing.
#[derive(Debug, Default)]
pub struct Encoder {
    /// Where the terminal cursor is, if known.
    cursor: Option<(u16, u16)>,
    /// The SGR state the terminal is in (default after every frame).
    pen: Style,
    /// The OSC 8 target currently open, if any.
    link: Option<Arc<str>>,
    /// Whether the terminal renders `SGR 4:x` and `SGR 58`.
    styled_underlines: bool,
}

impl Encoder {
    /// An encoder that knows nothing about the terminal yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Write `headline` as one `OSC 66` run — the scale, the fitted text,
    /// ST — at the cursor, in the pen. Both are the caller's: the screen
    /// moves to [`Headline::rect`] and sets [`Headline::style`] before
    /// this and resets the pen after it. The run prints text of its own,
    /// so where the cursor ends up is the terminal's business and the
    /// caller forgets it ([`Encoder::reset`]).
    pub fn headline(&self, headline: &Headline, out: &mut Vec<u8>) {
        out.extend_from_slice(b"\x1b]66;s=");
        push_number(out, u32::from(Headline::ROWS));
        out.push(b';');
        out.extend_from_slice(headline.text().as_bytes());
        out.extend_from_slice(b"\x1b\\");
    }

    /// Emit `SGR 4:x` underline styles and `SGR 58` underline colours
    /// from now on — for a terminal the allowlist
    /// ([`Env::styled_underlines`](crate::graphics::Env::styled_underlines))
    /// vouches for.
    ///
    /// **Off by default.** Every other terminal is sent the plain `SGR 4`
    /// for a curly, dotted, dashed or double rule and no rule colour at
    /// all, because [`Attrs`] carries the underline bit alongside the
    /// style; a terminal that does not know `4:3` gets an underline
    /// rather than a printed `:3`. Off by default is also what keeps
    /// every byte-exact capture and every golden where it was.
    pub const fn enable_underline_styles(&mut self) {
        self.styled_underlines = true;
    }

    /// Forget the cursor: call after anything else wrote to the terminal.
    pub fn reset(&mut self) {
        self.cursor = None;
    }

    /// Append one frame to `out`: BSU, optionally a full clear, the
    /// changes, an SGR reset when needed, ESU.
    ///
    /// `cols` is the terminal width, used to stop trusting the cursor once
    /// a write reaches the right edge (pending-wrap state is undefined).
    pub fn encode(&mut self, cols: u16, clear: bool, changes: &[Change<'_>], out: &mut Vec<u8>) {
        self.begin_frame(cols, clear, changes, out);
        Self::end_frame(out);
    }

    /// The frame up to (not including) ESU: BSU, optionally a full clear,
    /// the changes, an SGR reset when needed. Anything else that belongs
    /// inside the bracket — pixel placements — goes between this and
    /// [`Encoder::end_frame`].
    pub fn begin_frame(
        &mut self,
        cols: u16,
        clear: bool,
        changes: &[Change<'_>],
        out: &mut Vec<u8>,
    ) {
        out.extend_from_slice(BSU);
        if clear {
            out.extend_from_slice(CLEAR);
            self.cursor = None;
        }
        for change in changes {
            self.move_to(change.col, change.row, out);
            self.set_pen(&change.cell.style, out);
            self.set_link(change.cell.style.link.as_ref(), out);
            out.extend_from_slice(change.cell.symbol.as_bytes());
            let next_col = change.col.saturating_add(change.cell.width());
            self.cursor = (next_col < cols && agreed_width(&change.cell.symbol))
                .then_some((next_col, change.row));
        }
        self.set_link(None, out);
        self.reset_pen(out);
    }

    /// Bring the terminal back to the default pen, writing nothing when
    /// it is already there.
    ///
    /// Every frame ends here, and so does anything written inside the
    /// bracket after the cells — a [`Headline`](super::Headline)'s
    /// `OSC 66` run, which the screen writes in its own pen. The
    /// protocol scopes a run's *scale* to the escape code's own payload,
    /// so there is no size to turn off; what would otherwise leak past
    /// ESU is the pen, and this is what stops it.
    pub(crate) fn reset_pen(&mut self, out: &mut Vec<u8>) {
        if !self.pen.is_default_sgr() {
            out.extend_from_slice(SGR_RESET);
            self.pen = Style::default();
        }
    }

    /// Close the bracket (ESU).
    pub fn end_frame(out: &mut Vec<u8>) {
        out.extend_from_slice(ESU);
    }

    /// Put the cursor at `(col, row)` (zero-based), writing nothing when it
    /// is already there.
    pub(crate) fn move_to(&mut self, col: u16, row: u16, out: &mut Vec<u8>) {
        if self.cursor == Some((col, row)) {
            return;
        }
        out.extend_from_slice(b"\x1b[");
        push_number(out, u32::from(row) + 1);
        out.push(b';');
        push_number(out, u32::from(col) + 1);
        out.push(b'H');
        self.cursor = Some((col, row));
    }

    /// Bring the terminal's SGR state to `style`.
    ///
    /// The diff is all-or-nothing: when the pen in force is not the
    /// default one, a leading `0` resets it and the whole new pen
    /// follows. `SGR 0` resets the underline style and the underline
    /// colour along with the rest, so `4:0` and `59` never need to reach
    /// the wire — the reset that clears a curly error rule is the same
    /// `\x1b[0m` that clears its colour, and the frame's own trailing
    /// reset before ESU clears both for the next frame.
    pub(crate) fn set_pen(&mut self, style: &Style, out: &mut Vec<u8>) {
        // A terminal that was never told about SGR 58 is not wearing a
        // rule colour, so the diff must not chase one: without the
        // capability the effective colour is the default and the pen
        // settles instead of re-emitting an SGR per cell.
        let underline_color = if self.styled_underlines {
            style.underline_color
        } else {
            Color::Default
        };
        if self.pen.fg == style.fg
            && self.pen.bg == style.bg
            && self.pen.attrs == style.attrs
            && self.pen.underline_color == underline_color
        {
            return;
        }
        // Every parameter is pushed with a leading ';'. Starting from the
        // default pen no reset is needed, so the first ';' is dropped.
        out.extend_from_slice(b"\x1b[");
        let params = out.len();
        let needs_reset = !self.pen.is_default_sgr();
        if needs_reset {
            out.push(b'0');
        }
        push_attrs(out, style.attrs, self.styled_underlines);
        push_color(out, 38, style.fg);
        push_color(out, 48, style.bg);
        push_color(out, 58, underline_color);
        if !needs_reset {
            out.remove(params);
        }
        out.push(b'm');
        self.pen = Style {
            fg: style.fg,
            bg: style.bg,
            attrs: style.attrs,
            underline_color,
            link: None,
        };
    }

    fn set_link(&mut self, link: Option<&Arc<str>>, out: &mut Vec<u8>) {
        if self.link.as_ref() == link {
            return;
        }
        match link {
            Some(url) => {
                out.extend_from_slice(b"\x1b]8;;");
                out.extend_from_slice(url.as_bytes());
                out.extend_from_slice(b"\x1b\\");
            }
            None => out.extend_from_slice(LINK_CLOSE),
        }
        self.link = link.cloned();
    }
}

/// The attributes in SGR-code order, the underline keeping its place
/// between italic and reverse so nothing but the rule's own parameter
/// moves. A `styled` terminal gets `4:x` for anything but a plain rule;
/// everyone else gets `4`, which is what the underline bit riding along
/// with every style is for.
fn push_attrs(out: &mut Vec<u8>, attrs: Attrs, styled: bool) {
    if attrs.contains(Attrs::BOLD) {
        out.extend_from_slice(b";1");
    }
    if attrs.contains(Attrs::DIM) {
        out.extend_from_slice(b";2");
    }
    if attrs.contains(Attrs::ITALIC) {
        out.extend_from_slice(b";3");
    }
    match attrs.underline() {
        None => {}
        Some(style) if styled && style != Underline::Single => {
            out.extend_from_slice(b";4:");
            push_number(out, u32::from(style.code()));
        }
        Some(_) => out.extend_from_slice(b";4"),
    }
    if attrs.contains(Attrs::REVERSE) {
        out.extend_from_slice(b";7");
    }
    if attrs.contains(Attrs::STRIKETHROUGH) {
        out.extend_from_slice(b";9");
    }
}

/// `base` is 38 for foreground, 48 for background, 58 for the underline
/// rule.
fn push_color(out: &mut Vec<u8>, base: u32, color: Color) {
    match color {
        Color::Default => {}
        Color::Indexed(n) => {
            out.push(b';');
            push_number(out, base);
            out.extend_from_slice(b";5;");
            push_number(out, u32::from(n));
        }
        Color::Rgb(r, g, b) => {
            out.push(b';');
            push_number(out, base);
            out.extend_from_slice(b";2;");
            push_number(out, u32::from(r));
            out.push(b';');
            push_number(out, u32::from(g));
            out.push(b';');
            push_number(out, u32::from(b));
        }
    }
}

/// Whether every terminal advances the cursor by exactly the columns
/// [`width`](super::width) measures `symbol` at — the condition for
/// trusting the cursor after writing it.
///
/// Terminals answer 2, 4, 5 or 6 columns for the same ZWJ sequence, join
/// or do not join a flag pair, and render a VS16 or a lone emoji narrow
/// or wide; the only graphemes they all agree on are a single scalar of
/// width 1 and a single East Asian Wide/Fullwidth scalar (CJK). After
/// anything else the cursor is forgotten, so the next change in the run
/// starts with an explicit CUP — a few bytes, only when something follows.
fn agreed_width(symbol: &str) -> bool {
    let mut scalars = symbol.chars();
    let (Some(first), None) = (scalars.next(), scalars.next()) else {
        return false;
    };
    // CJK and the other East Asian Wide / Fullwidth ranges.
    let east_asian_wide = matches!(
        first,
        '\u{1100}'..='\u{115F}'
            | '\u{2E80}'..='\u{A4CF}'
            | '\u{AC00}'..='\u{D7A3}'
            | '\u{F900}'..='\u{FAFF}'
            | '\u{FE30}'..='\u{FE4F}'
            | '\u{FF00}'..='\u{FF60}'
            | '\u{FFE0}'..='\u{FFE6}'
            | '\u{20000}'..='\u{3FFFD}'
    );
    match magistr_cockpit::render::width(symbol) {
        1 => true,
        2 => east_asian_wide,
        _ => false,
    }
}

/// Append `n` in decimal (no allocation, no formatting machinery).
pub(crate) fn push_number(out: &mut Vec<u8>, mut n: u32) {
    let mut digits = [0u8; 10];
    let mut start = digits.len();
    loop {
        start -= 1;
        digits[start] = b'0' + u8::try_from(n % 10).unwrap_or(0);
        n /= 10;
        if n == 0 {
            break;
        }
    }
    out.extend_from_slice(&digits[start..]);
}

#[cfg(test)]
#[path = "encode.tests.rs"]
mod tests;
