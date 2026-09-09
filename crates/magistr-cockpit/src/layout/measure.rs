//! How the engine asks what a text leaf is worth.

use super::text::TextMetrics;

/// The seam between the layout engine and whatever knows how wide a
/// string is.
///
/// A terminal answers in columns and every row is one cell tall
/// ([`CellMeasure`](super::CellMeasure)). The native window measures
/// through its own element substrate and never reaches this seam.
///
/// [`Measure::wrap`] is the load-bearing verb: the painter calls it too,
/// so the rows it draws are by construction the rows the layout
/// measured. Two greedy wrappers that merely agreed would be a bug
/// waiting for a font change.
pub trait Measure {
    /// The widest line and the widest word of `text`, in layout
    /// units — flexbox's max-content and min-content.
    fn intrinsic(&mut self, text: &TextMetrics) -> (f32, f32);

    /// `text` wrapped to `width` units, one entry per row.
    fn wrap<'a>(&mut self, text: &'a TextMetrics, width: f32) -> Vec<&'a str>;

    /// The height of one row, in layout units.
    fn line_height(&mut self) -> f32;

    /// The units `text` needs vertically at `width` — rows times row
    /// height. Derived rather than answered separately, so a measurer
    /// cannot report a height its own wrap disagrees with.
    fn height(&mut self, text: &TextMetrics, width: f32) -> f32 {
        let rows = self.wrap(text, width).len();
        #[allow(clippy::cast_precision_loss)]
        let rows = rows as f32;
        rows * self.line_height()
    }
}
