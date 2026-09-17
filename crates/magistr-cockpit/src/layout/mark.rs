//! What a node asks the pixel layer to draw over its cells.

use crate::pixel::Icon;
use crate::render::Color;

/// A pixel decoration a node carries: what the view spells in glyphs
/// on a plain terminal, described for the compositor to draw on a
/// graphics one. The node's text keeps a blank where a mark draws, so
/// the layout is the same width on both paths and the cell goldens
/// never move; the compositor places every mark under the text, so a
/// label, a key or a title reads over the surface it sits on.
///
/// A mark is a description and nothing else — no protocol, no pixels —
/// which is what keeps the cockpit independent of the terminal it is
/// composited on. [`marks`](super::marks) pairs each with the rect
/// layout gave its node.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Mark {
    /// A vocabulary glyph over `cols` cells from column `col` of the
    /// node's first row, drawn in `ink` on a transparent ground.
    Icon {
        /// Which glyph.
        icon: Icon,
        /// The cell it starts at, from the node's left edge.
        col: u16,
        /// How many cells it may use — one for a mark in a line of
        /// text, two for a button's leading glyph.
        cols: u16,
        /// Its colour, keyed to the status vocabulary by the pen.
        ink: Color,
    },
    /// A raised key cap under the node's whole rect: `fill` rounded on
    /// `ground`, the bar the cap sits on.
    Chip {
        /// The cap's colour.
        fill: Color,
        /// The bar's colour, where the cap's corners are rounded off.
        ground: Color,
    },
    /// One segment of a segmented control: a `track` under the node's
    /// rect, rounded at the control's outer ends, and a `fill` pill
    /// when this segment is the current one.
    Segment {
        /// The current segment's pill, `None` for the others.
        fill: Option<Color>,
        /// The track every segment shares.
        track: Color,
        /// The strip the control sits on.
        ground: Color,
        /// Whether this is the control's first segment.
        first: bool,
        /// Whether this is its last.
        last: bool,
    },
    /// A fraction as a smooth bar over the node's rect.
    Bar {
        /// The fraction, in thousandths (`0..=1000`).
        fill: u16,
        /// The bar's colour; the track is the same colour, faint.
        line: Color,
        /// The ground behind the track.
        ground: Color,
    },
    /// Unknown progress: a fixed-length segment moving along a track.
    Indeterminate {
        /// Segment position in thousandths, independent of any completion fraction.
        position: u16,
        /// The segment's colour.
        line: Color,
        /// The ground behind the track.
        ground: Color,
    },
    /// A soft horizontal rule across the node's rect.
    Rule {
        /// The rule's colour.
        ink: Color,
    },
    /// A drawn panel ring over the node's rect — a framed node whose
    /// [`Frame`](super::Frame) is drawn rather than spelled — with the
    /// columns its title occupies on the top edge, from the rect's
    /// left, so the ring parts around the label.
    Panel {
        /// The ring's colour.
        border: Color,
        /// `(col, cols)` of the title on the top row, if there is one.
        title: Option<(u16, u16)>,
    },
}

#[cfg(test)]
#[path = "mark.tests.rs"]
mod tests;
