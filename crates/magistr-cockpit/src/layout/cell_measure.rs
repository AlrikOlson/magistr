//! Measuring text in terminal columns.

use super::measure::Measure;
use super::text::TextMetrics;

/// The terminal's measurer: a column is one unit, a row is one unit
/// tall, and a role means nothing because a cell is a cell.
///
/// This is the identity case of the whole pixel-layout mechanism — on
/// [`Basis::CELLS`](super::Basis::CELLS) with this measurer the engine
/// does the arithmetic it did before either existed, which is what
/// makes the terminal's byte-identity structural.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CellMeasure;

impl Measure for CellMeasure {
    fn intrinsic(&mut self, text: &TextMetrics) -> (f32, f32) {
        (
            f32::from(text.max_content_width()),
            f32::from(text.min_content_width()),
        )
    }

    fn wrap<'a>(&mut self, text: &'a TextMetrics, width: f32) -> Vec<&'a str> {
        text.wrap(columns(width))
    }

    fn line_height(&mut self) -> f32 {
        1.0
    }
}

/// Whole columns available at `width` units. Floored, because half a
/// column holds no glyph.
fn columns(width: f32) -> u16 {
    width.floor().clamp(0.0, f32::from(u16::MAX)) as u16
}

#[cfg(test)]
#[path = "cell_measure.tests.rs"]
mod tests;
