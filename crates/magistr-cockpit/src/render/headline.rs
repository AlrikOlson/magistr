//! A run of scaled text: kitty's text sizing protocol, one headline.

use unicode_segmentation::UnicodeSegmentation;

use super::style::Style;
use super::width::width;
use crate::layout::Rect;

/// The scale every headline is drawn at: two cells wide and two rows
/// tall per character, the only size a two-row slot has room for.
const SCALE: u16 = 2;

/// One run of double-height text placed on the grid — on a terminal,
/// through kitty's text sizing protocol (`OSC 66`), for the terminals
/// the startup probe found it on ([`Msg::TextSizing`](crate::cockpit::Msg::TextSizing)).
///
/// A headline is *frame-level* content, like a pixel placement and
/// unlike a cell: the terminal's screen declares it for the frame being
/// drawn and emits it after the cells and the surfaces, so the run lands
/// on top of the rows the view left blank for it. The terminal draws the
/// block itself — the cursor moves `s × w` cells to the right, in the
/// row it was in — so the second row belongs to the terminal and must
/// hold nothing of ours.
///
/// [`Headline::new`] fits the text to the rect it was given: at most
/// `width / 2` characters, then **padded** back out to that many. The
/// padding is what makes a headline idempotent to replace — a shorter
/// name paints scaled spaces over the one before it, so nothing has to
/// erase the block first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Headline {
    text: String,
    rect: Rect,
    style: Style,
}

impl Headline {
    /// Rows a headline covers: the scale, in cells.
    pub const ROWS: u16 = SCALE;

    /// The headline `text` fills `rect` with, in `style` — cut to the
    /// characters the rect has room for at [`Headline::ROWS`]-times
    /// scale, then padded back out to them.
    ///
    /// `None` when the rect is too narrow or too short for one scaled
    /// character: a slot that cannot hold a headline shows none rather
    /// than a clipped one.
    #[must_use]
    pub fn fill(text: &str, rect: Rect, style: Style) -> Option<Self> {
        let cells = usize::from(rect.width / SCALE);
        if cells == 0 || rect.height < SCALE {
            return None;
        }
        let mut fitted = String::new();
        let mut taken = 0usize;
        for grapheme in text.graphemes(true) {
            let cost = usize::from(width(grapheme));
            if taken + cost > cells {
                break;
            }
            taken += cost;
            fitted.push_str(grapheme);
        }
        for _ in taken..cells {
            fitted.push(' ');
        }
        Some(Self {
            text: fitted,
            rect,
            style,
        })
    }

    /// Where the run starts and what it covers.
    #[must_use]
    pub const fn rect(&self) -> Rect {
        self.rect
    }

    /// The pen the run is written in.
    #[must_use]
    pub const fn style(&self) -> &Style {
        &self.style
    }

    /// The fitted, padded run: exactly `rect.width / ROWS` cells of
    /// text, which is what a renderer scales across the slot.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
}

#[cfg(test)]
#[path = "headline.tests.rs"]
mod tests;
