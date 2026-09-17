use magistr_cockpit::cockpit::Size;
use magistr_cockpit::layout::Rect;
use qs_text::fontdb::FontId;
use qs_text::shape::Shaper;
use qs_text::{Features, PxSize};

/// Pixel metrics for the cockpit's cell-based content.
///
/// [`CellMetrics::rect`] maps model cell coordinates into pixels. The native component
/// layout uses its own pixel geometry.
///
/// Cell dimensions are integral so adjacent columns share consistent pixel edges. The
/// advance is measured by shaping `M`, allowing the grid to follow the embedded font's
/// metrics.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellMetrics {
    font: FontId,
    size: PxSize,
    width: f32,
    height: f32,
    baseline: f32,
}

impl CellMetrics {
    /// The size the cockpit's face is drawn at when nothing says
    /// otherwise. 15 px is what the screenshot gate rasterizes the
    /// terminal stills at (`screenshot::Typeface::PX`), so the native
    /// window and the terminal goldens are the same face at the same
    /// size and a difference between them is a difference in *painting*.
    pub const DEFAULT_PX: f32 = 15.0;

    /// The grid `font` makes at `px`, or `None` when the face has no
    /// usable metrics (a zero units-per-em, or bytes that do not parse).
    #[must_use]
    pub fn new(shaper: &mut Shaper, font: FontId, px: f32) -> Option<Self> {
        let size = PxSize::new(px);
        let metrics = shaper.metrics(font, size)?;
        // The advance of one glyph on a monospaced face is the column
        // width. Measured rather than taken from `units_per_em * 0.6`,
        // which is JetBrains Mono's number and not a fact about faces.
        let advance = shaper.shape("M", font, size, Features::default()).width;
        let width = advance.round().max(1.0);
        let height = metrics.line_height.round().max(1.0);
        // The baseline is the face's ascent, floored into the cell: a
        // descender that would fall outside the cell is clipped by the
        // row below it rather than pushing the grid open.
        let baseline = metrics.ascent.round().clamp(1.0, height);
        Some(Self {
            font,
            size,
            width,
            height,
            baseline,
        })
    }

    /// The face the grid was measured from.
    #[must_use]
    pub const fn font(&self) -> FontId {
        self.font
    }

    /// The pixel size runs are shaped at.
    #[must_use]
    pub const fn size(&self) -> PxSize {
        self.size
    }

    /// One column, in physical pixels.
    #[must_use]
    pub const fn width(&self) -> f32 {
        self.width
    }

    /// One row, in physical pixels.
    #[must_use]
    pub const fn height(&self) -> f32 {
        self.height
    }

    /// The cell as the cockpit's own `CellSize` counts it — whole
    /// pixels a side, which `width` and `height` already are — and as
    /// the model is told it (`Reply::CellPixels`). One place for the
    /// conversion, so the cell the turn measures an image on and the
    /// cell the chrome paints its quad on cannot round differently.
    #[must_use]
    pub const fn cell_pixels(&self) -> (u16, u16) {
        (self.width as u16, self.height as u16)
    }

    /// Pixels from a row's top to the baseline its glyphs sit on.
    #[must_use]
    pub const fn baseline(&self) -> f32 {
        self.baseline
    }

    /// The left edge of column `col`.
    #[must_use]
    pub fn x(&self, col: u16) -> f32 {
        f32::from(col) * self.width
    }

    /// The top edge of row `row`.
    #[must_use]
    pub fn y(&self, row: u16) -> f32 {
        f32::from(row) * self.height
    }

    /// A cell rect as `[x, y, w, h]` in physical pixels.
    #[must_use]
    pub fn rect(&self, rect: Rect) -> [f32; 4] {
        [
            self.x(rect.x),
            self.y(rect.y),
            f32::from(rect.width) * self.width,
            f32::from(rect.height) * self.height,
        ]
    }

    /// The scissor a rect asks for, as the `[x, y, w, h]` unsigned
    /// rectangle a [`DrawList`](qs_gpu::frame::DrawList) batch carries.
    #[must_use]
    pub fn scissor(&self, rect: Rect) -> [u32; 4] {
        let [x, y, w, h] = self.rect(rect);
        [x as u32, y as u32, w.ceil() as u32, h.ceil() as u32]
    }

    /// How many whole cells fit a `width` x `height` viewport, as the
    /// cockpit's own [`Size`]. Never zero on either axis: a `Model` with
    /// no columns has nothing to lay out, and a window mid-resize
    /// legitimately reports one pixel.
    #[must_use]
    pub fn grid(&self, width: u32, height: u32) -> Size {
        let cols = (width as f32 / self.width).floor().max(1.0);
        let rows = (height as f32 / self.height).floor().max(1.0);
        Size::new(cols as u16, rows as u16)
    }

    /// The cell a physical-pixel position lands in, clamped into
    /// `size`.
    ///
    /// The inverse of [`CellMetrics::x`]/[`CellMetrics::y`] and the
    /// whole of "pixel → cell": past it the pointer is in the cockpit's
    /// own coordinates and the terminal's
    /// [`HitRegistry`](magistr_cockpit::hit::HitRegistry) answers for it
    /// unchanged, which is why the native GUI needs no second hit path.
    #[must_use]
    pub fn cell_at(&self, x: f64, y: f64, size: Size) -> (u16, u16) {
        let col = (x / f64::from(self.width)).floor().max(0.0) as u32;
        let row = (y / f64::from(self.height)).floor().max(0.0) as u32;
        (
            col.min(u32::from(size.cols.saturating_sub(1))) as u16,
            row.min(u32::from(size.rows.saturating_sub(1))) as u16,
        )
    }
}

#[cfg(test)]
#[path = "metrics.tests.rs"]
mod tests;
