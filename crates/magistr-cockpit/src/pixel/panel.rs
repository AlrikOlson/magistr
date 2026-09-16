//! A framed node's ring as a drawn panel.

use tiny_skia::{
    Color, FillRule, LineCap, LineJoin, Paint, PathBuilder, PixmapMut, Stroke, Transform,
};

use super::cell_size::CellSize;
use super::painter::Painter;
use super::rounded::rounded;

/// The ring's line width in pixels.
const LINE: f32 = 1.5;
/// The corner radius as a fraction of the shorter cell side.
const RADIUS: f32 = 0.9;
/// The label pill's alpha in the border colour, under the title text.
const LABEL_ALPHA: f32 = 0.42;
/// Pixels the ring parts from the label on each side.
const LABEL_GAP: f32 = 3.0;

/// A panel's ring drawn in pixels over the cells the layout reserved for
/// its border: a soft rounded line running through the middle of the
/// ring cells, on a transparent ground so the cells inside keep their
/// own backgrounds, parting around the title on the top edge, where a
/// label pill sits under the title's text — paint it into a
/// [`PixelSurface::with_alpha`](super::PixelSurface::with_alpha)
/// surface covering the framed node's whole rect.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Panel {
    border: Color,
    cell: CellSize,
    title: Option<(u16, u16)>,
}

impl Panel {
    /// A ring in `(r, g, b)` on a grid of `cell`-sized cells.
    #[must_use]
    pub fn new(r: u8, g: u8, b: u8, cell: CellSize) -> Self {
        Self {
            border: Color::from_rgba8(r, g, b, 255),
            cell,
            title: None,
        }
    }

    /// Part the top edge around a title `cols` cells wide from `col`.
    #[must_use]
    pub const fn title(mut self, title: Option<(u16, u16)>) -> Self {
        self.title = title;
        self
    }
}

impl Painter for Panel {
    fn paint(&self, pixmap: &mut PixmapMut<'_>) {
        pixmap.fill(Color::TRANSPARENT);
        let (width, height) = (pixmap.width() as f32, pixmap.height() as f32);
        let (cw, ch) = (f32::from(self.cell.width), f32::from(self.cell.height));
        if width < 2.0 * cw || height < 2.0 * ch {
            return;
        }
        let (left, top) = (cw / 2.0, ch / 2.0);
        let (right, bottom) = (width - cw / 2.0, height - ch / 2.0);
        let radius = (cw.min(ch) * RADIUS)
            .min((right - left) / 2.0)
            .min((bottom - top) / 2.0);
        let mut paint = Paint {
            anti_alias: true,
            ..Paint::default()
        };
        paint.set_color(self.border);
        let stroke = Stroke {
            width: LINE,
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            ..Stroke::default()
        };
        // The label: where the top edge parts, and the pill under it.
        let label = self.title.map(|(col, cols)| {
            let x0 = f32::from(col) * cw;
            let x1 = f32::from(col + cols) * cw;
            (x0.min(right - radius), x1.min(right - radius))
        });
        let mut path = PathBuilder::new();
        let start = label.map_or(left + radius, |(_, x1)| x1 + LABEL_GAP);
        path.move_to(start, top);
        path.line_to(right - radius, top);
        path.quad_to(right, top, right, top + radius);
        path.line_to(right, bottom - radius);
        path.quad_to(right, bottom, right - radius, bottom);
        path.line_to(left + radius, bottom);
        path.quad_to(left, bottom, left, bottom - radius);
        path.line_to(left, top + radius);
        path.quad_to(left, top, left + radius, top);
        match label {
            Some((x0, _)) => path.line_to((x0 - LABEL_GAP).max(left + radius), top),
            None => path.close(),
        }
        if let Some(path) = path.finish() {
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
        if let Some((x0, x1)) = label.filter(|(x0, x1)| x1 > x0) {
            let mut fill = self.border;
            fill.set_alpha(LABEL_ALPHA);
            paint.set_color(fill);
            let inset = ch * 0.12;
            if let Some(pill) = rounded(x0, inset, x1, ch - inset, (ch / 2.0 - inset).min(4.0)) {
                pixmap.fill_path(
                    &pill,
                    &paint,
                    FillRule::Winding,
                    Transform::identity(),
                    None,
                );
            }
        }
    }
}

#[cfg(test)]
#[path = "panel.tests.rs"]
mod tests;
