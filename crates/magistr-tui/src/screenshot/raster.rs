//! A cell buffer as the pixels a terminal would show.

use magistr_theme::Palette;
use tiny_skia::{IntSize, Pixmap};

use super::boxdraw;
use super::glyph::Glyph;
use super::placement::Placement;
use super::typeface::Typeface;
use magistr_cockpit::render::resolve;
use magistr_cockpit::render::{Attrs, Buffer, Cell, Color, Underline};

/// Dim text keeps this percentage of its own colour; the rest is the
/// background's.
const DIM_KEEP: u32 = 45;
/// Italic slides each row of a glyph right by this percentage of its
/// height above the baseline (and left below it).
const ITALIC_SHEAR_PERCENT: i32 = 25;
/// The coverage a missing glyph's box is drawn at: visible, not shouting.
const TOFU: u8 = 110;
/// The underline sits this many pixels under the baseline.
const UNDERLINE_DROP: u32 = 2;
/// A curly rule's row offsets from one pixel above the drop, over a
/// four-pixel period: up, middle, down, middle — the wave an editor
/// draws under a diagnostic.
const CURLY_WAVE: [u32; 4] = [0, 1, 2, 1];
/// A dotted rule paints one pixel of every this many.
const DOTTED_PERIOD: usize = 2;
/// A dashed rule paints this many pixels of every [`DASHED_PERIOD`].
const DASHED_ON: u32 = 3;
/// A dashed rule's period.
const DASHED_PERIOD: u32 = 5;

/// Every cell of `buffer` as opaque pixels, one [`Typeface`] cell each:
/// the background first (the theme's base for the default colour),
/// then the grapheme's first scalar drawn in the foreground — the bold
/// face for `BOLD`, sheared for `ITALIC`, a hollow box when the face has
/// no outline for it — then the underline and `STRIKETHROUGH` rules. An
/// underline is drawn in its [`Underline`] style and in
/// [`Style::underline_color`] when the cell names one, so a PNG golden
/// shows the same rule the terminal would.
/// `REVERSE` swaps the two colours, `DIM` pulls the foreground toward the
/// background. A wide grapheme covers its continuation cells; a
/// zero-sized buffer is one pixel, like an empty pixel surface.
///
/// `images` are the kitty placements the same frame put on screen
/// (empty for a cell-only terminal). They go on between the two passes
/// because that is where the protocol puts them: `z=-1` is over the
/// cells' background and under their text, and
/// [`Kitty::Z_UNDER_BACKGROUND`](crate::graphics::Kitty::Z_UNDER_BACKGROUND)
/// is the separate z the cockpit never uses. Splitting the cells into a
/// background pass and a glyph pass is what makes the sandwich
/// possible, and it changes nothing without images: [`blit`] clips
/// every glyph pixel to its own cell's span, so no cell's background
/// could ever have erased a neighbour's ink in the single-pass version
/// either.
#[must_use]
pub fn rasterize(
    buffer: &Buffer,
    palette: &Palette,
    face: &mut Typeface,
    images: &[Placement],
) -> Pixmap {
    let cell_h = Typeface::CELL_HEIGHT;
    let mut canvas = Canvas::new(
        u32::from(buffer.width()) * Typeface::CELL_WIDTH,
        u32::from(buffer.height()) * cell_h,
    );
    for cell in cells(buffer) {
        let (_, bg) = pen(cell.cell, palette);
        canvas.fill(cell.x, cell.y, cell.span, cell_h, bg);
    }
    for image in images {
        canvas.image(image);
    }
    for cell in cells(buffer) {
        let (fg, _) = pen(cell.cell, palette);
        symbol(&mut canvas, face, cell.cell, cell.x, cell.y, cell.span, fg);
        let baseline = face.baseline();
        if let Some(style) = cell.cell.style.attrs.underline() {
            let rule = match cell.cell.style.underline_color {
                Color::Default => fg,
                color => resolve(color, palette, true),
            };
            underline(
                &mut canvas,
                baseline,
                style,
                cell.x,
                cell.y,
                cell.span,
                rule,
            );
        }
        rules(
            &mut canvas,
            baseline,
            cell.cell.style.attrs,
            cell.x,
            cell.y,
            cell.span,
            fg,
        );
    }
    canvas.into_pixmap()
}

/// One cell of the buffer with the pixels it occupies.
struct Placed<'a> {
    cell: &'a Cell,
    x: u32,
    y: u32,
    span: u32,
}

/// Every cell that draws something, in row-major order — the
/// continuations of a wide grapheme skipped, since the grapheme's own
/// cell already covers them.
fn cells(buffer: &Buffer) -> impl Iterator<Item = Placed<'_>> {
    (0..buffer.height()).flat_map(move |row| {
        (0..buffer.width()).filter_map(move |col| {
            let cell = buffer.get(col, row)?;
            (!cell.is_continuation()).then(|| Placed {
                cell,
                x: u32::from(col) * Typeface::CELL_WIDTH,
                y: u32::from(row) * Typeface::CELL_HEIGHT,
                span: u32::from(cell.width().max(1)) * Typeface::CELL_WIDTH,
            })
        })
    })
}

/// The colours a cell is drawn in after `REVERSE` and `DIM`.
fn pen(cell: &Cell, palette: &Palette) -> ([u8; 3], [u8; 3]) {
    let attrs = cell.style.attrs;
    let mut fg = resolve(cell.style.fg, palette, true);
    let mut bg = resolve(cell.style.bg, palette, false);
    if attrs.contains(Attrs::REVERSE) {
        std::mem::swap(&mut fg, &mut bg);
    }
    if attrs.contains(Attrs::DIM) {
        fg = mix(bg, fg, DIM_KEEP);
    }
    (fg, bg)
}

/// `percent` of `b` over `a`, per channel.
fn mix(a: [u8; 3], b: [u8; 3], percent: u32) -> [u8; 3] {
    let channel = |i: usize| {
        let v = (u32::from(a[i]) * (100 - percent) + u32::from(b[i]) * percent + 50) / 100;
        u8::try_from(v).unwrap_or(u8::MAX)
    };
    [channel(0), channel(1), channel(2)]
}

/// Draw the cell's grapheme into its `span`-wide cell at `(x, y)`.
///
/// Box-drawing and block glyphs are drawn from the cell's geometry
/// ([`boxdraw`]) rather than asked of the face, the way kitty and WezTerm
/// do it: the face advances less than a full cell, so its `─` left a
/// column of background at every boundary and a panel ring read as a
/// dashed line. A synthesized one reaches both edges, and
/// neither weight nor the italic shear applies to it — a box-drawing
/// glyph is geometry, not typography.
fn symbol(
    canvas: &mut Canvas,
    face: &mut Typeface,
    cell: &Cell,
    x: u32,
    y: u32,
    span: u32,
    fg: [u8; 3],
) {
    let Some(ch) = cell.symbol.chars().next() else {
        return;
    };
    if ch.is_whitespace() {
        return;
    }
    if let Some(shape) = boxdraw::glyph(ch) {
        blit(canvas, &shape, x, y, span, fg, None);
        return;
    }
    if !face.has_glyph(ch) {
        canvas.tofu(x, y, span, Typeface::CELL_HEIGHT, fg);
        return;
    }
    let attrs = cell.style.attrs;
    let baseline = attrs
        .contains(Attrs::ITALIC)
        .then(|| i32::try_from(face.baseline()).unwrap_or(i32::MAX));
    let glyph = face.glyph(ch, attrs.contains(Attrs::BOLD));
    blit(canvas, glyph, x, y, span, fg, baseline);
}

/// Lay a glyph's coverage into the cell at `(x, y)`, clipped to it.
/// `italic` carries the baseline to shear each row about when the cell
/// is italic: right above it, left below it.
fn blit(
    canvas: &mut Canvas,
    glyph: &Glyph,
    x: u32,
    y: u32,
    span: u32,
    fg: [u8; 3],
    italic: Option<i32>,
) {
    let (left, top) = (
        i32::try_from(x).unwrap_or(i32::MAX),
        i32::try_from(y).unwrap_or(i32::MAX),
    );
    let right = left.saturating_add(i32::try_from(span).unwrap_or(i32::MAX));
    let bottom = top.saturating_add(i32::try_from(Typeface::CELL_HEIGHT).unwrap_or(i32::MAX));
    for gy in 0..glyph.height {
        let row = glyph.top + i32::try_from(gy).unwrap_or(i32::MAX);
        let py = top + row;
        if py < top || py >= bottom {
            continue;
        }
        let shear = italic.map_or(0, |baseline| (baseline - row) * ITALIC_SHEAR_PERCENT / 100);
        for gx in 0..glyph.width {
            let coverage = glyph.coverage[gy * glyph.width + gx];
            if coverage == 0 {
                continue;
            }
            let px = left + glyph.left + i32::try_from(gx).unwrap_or(i32::MAX) + shear;
            if px < left || px >= right {
                continue;
            }
            canvas.blend(px.unsigned_abs(), py.unsigned_abs(), fg, coverage);
        }
    }
}

/// The underline rule across the cell, in `style`'s shape and in
/// `rule`'s colour. The wave and the two rules of a double sit one pixel
/// either side of where a single one goes, so every style stays on the
/// cell whatever the face's baseline is.
fn underline(
    canvas: &mut Canvas,
    baseline: u32,
    style: Underline,
    x: u32,
    y: u32,
    span: u32,
    rule: [u8; 3],
) {
    let mid = (baseline + UNDERLINE_DROP).clamp(1, Typeface::CELL_HEIGHT - 2);
    match style {
        Underline::Single => canvas.fill(x, y + mid, span, 1, rule),
        Underline::Double => {
            canvas.fill(x, y + mid - 1, span, 1, rule);
            canvas.fill(x, y + mid + 1, span, 1, rule);
        }
        Underline::Curly => {
            for px in 0..span {
                let row = mid - 1 + CURLY_WAVE[usize::try_from(px % 4).unwrap_or(0)];
                canvas.fill(x + px, y + row, 1, 1, rule);
            }
        }
        Underline::Dotted => {
            for px in (0..span).step_by(DOTTED_PERIOD) {
                canvas.fill(x + px, y + mid, 1, 1, rule);
            }
        }
        Underline::Dashed => {
            for px in (0..span).filter(|px| px % DASHED_PERIOD < DASHED_ON) {
                canvas.fill(x + px, y + mid, 1, 1, rule);
            }
        }
    }
}

/// The strikethrough rule across the cell — the underline is
/// [`underline`]'s, because it has a shape and a colour of its own.
fn rules(canvas: &mut Canvas, baseline: u32, attrs: Attrs, x: u32, y: u32, span: u32, fg: [u8; 3]) {
    if attrs.contains(Attrs::STRIKETHROUGH) {
        canvas.fill(x, y + baseline - baseline / 3, span, 1, fg);
    }
}

/// An opaque RGBA canvas the cells are composed on; straight and
/// premultiplied agree at full alpha, so it hands its bytes to tiny-skia
/// as they are.
struct Canvas {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl Canvas {
    /// A black canvas of at least one pixel.
    fn new(width: u32, height: u32) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        let pixels = usize::try_from(width * height).unwrap_or(usize::MAX);
        Self {
            width,
            height,
            rgba: [0, 0, 0, 255].repeat(pixels),
        }
    }

    /// The byte offset of `(x, y)`, when it is on the canvas.
    fn at(&self, x: u32, y: u32) -> Option<usize> {
        (x < self.width && y < self.height)
            .then(|| usize::try_from((y * self.width + x) * 4).unwrap_or(usize::MAX))
    }

    /// Set every pixel of the rect, clipped to the canvas.
    fn fill(&mut self, x: u32, y: u32, w: u32, h: u32, rgb: [u8; 3]) {
        for py in y..y.saturating_add(h).min(self.height) {
            for px in x..x.saturating_add(w).min(self.width) {
                if let Some(i) = self.at(px, py) {
                    self.rgba[i..i + 3].copy_from_slice(&rgb);
                }
            }
        }
    }

    /// Lay `rgb` over the pixel at `coverage` (0 none, 255 solid).
    fn blend(&mut self, x: u32, y: u32, rgb: [u8; 3], coverage: u8) {
        let Some(i) = self.at(x, y) else {
            return;
        };
        let cov = u32::from(coverage);
        for (c, &channel) in rgb.iter().enumerate() {
            let old = u32::from(self.rgba[i + c]);
            let new = (old * (255 - cov) + u32::from(channel) * cov + 127) / 255;
            self.rgba[i + c] = u8::try_from(new).unwrap_or(u8::MAX);
        }
    }

    /// Lay a kitty placement's pixels over the canvas, scaled from the
    /// image's own size into the cells it was placed across.
    ///
    /// The cockpit paints every surface at the model's cell size, which
    /// is 10x20 whenever the terminal never answered `CSI 16 t` — the
    /// same cell this rasterizer draws in — so the sampling is 1:1 in
    /// practice. It is written as a resample anyway, because a model
    /// carrying a measured cell size would otherwise be drawn at the
    /// wrong scale rather than at a visibly wrong one.
    fn image(&mut self, placement: &Placement) {
        let (w, h) = (placement.width, placement.height);
        if w == 0 || h == 0 {
            return;
        }
        let left = u32::from(placement.col) * Typeface::CELL_WIDTH;
        let top = u32::from(placement.row) * Typeface::CELL_HEIGHT;
        let span = u32::from(placement.cols) * Typeface::CELL_WIDTH;
        let down = u32::from(placement.rows) * Typeface::CELL_HEIGHT;
        for py in 0..down {
            for px in 0..span {
                let sx = (px * w / span.max(1)).min(w - 1);
                let sy = (py * h / down.max(1)).min(h - 1);
                let at = usize::try_from((sy * w + sx) * 4).unwrap_or(usize::MAX);
                let Some(pixel) = placement.rgba.get(at..at + 4) else {
                    continue;
                };
                self.blend(
                    left + px,
                    top + py,
                    [pixel[0], pixel[1], pixel[2]],
                    pixel[3],
                );
            }
        }
    }

    /// A hollow box inset from the cell's edges: what a terminal shows
    /// for a glyph its font lacks.
    fn tofu(&mut self, x: u32, y: u32, w: u32, h: u32, rgb: [u8; 3]) {
        let (left, right) = (x + 1, (x + w).saturating_sub(2));
        let (top, bottom) = (y + 2, (y + h).saturating_sub(3));
        for px in left..=right {
            self.blend(px, top, rgb, TOFU);
            self.blend(px, bottom, rgb, TOFU);
        }
        for py in top..=bottom {
            self.blend(left, py, rgb, TOFU);
            self.blend(right, py, rgb, TOFU);
        }
    }

    /// The canvas as a pixmap of its own size.
    fn into_pixmap(self) -> Pixmap {
        let size = IntSize::from_wh(self.width, self.height).expect("a canvas is never empty");
        Pixmap::from_vec(self.rgba, size).expect("the canvas holds exactly its pixels")
    }
}

#[cfg(test)]
#[path = "raster.tests.rs"]
mod tests;
