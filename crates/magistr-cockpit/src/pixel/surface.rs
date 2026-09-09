//! A rectangle of cells that a widget paints in pixels.

use tiny_skia::Pixmap;

use super::cell_size::CellSize;
use super::painter::Painter;
use super::pixel_format::PixelFormat;
use crate::layout::Rect;

/// A widget's pixel area: a [`Rect`] in cells, a pixmap of `rect ×
/// cell size` pixels, and a dirty flag the compositor clears when the
/// pixels reach the terminal.
///
/// Opaque by default — the wire carries three bytes per pixel (`f=24`),
/// a quarter fewer than RGBA, and premultiplied equals straight at full
/// alpha so no conversion is needed. [`PixelSurface::with_alpha`] makes a
/// surface that keeps its transparency (`f=32`, demultiplied on the way
/// out because kitty wants straight alpha).
#[derive(Clone, Debug)]
pub struct PixelSurface {
    rect: Rect,
    cell: CellSize,
    pixmap: Pixmap,
    opaque: bool,
    dirty: bool,
}

impl PixelSurface {
    /// An opaque surface covering `rect` at `cell` pixels per cell,
    /// transparent black until painted, dirty until transmitted.
    #[must_use]
    pub fn new(rect: Rect, cell: CellSize) -> Self {
        Self {
            rect,
            cell,
            pixmap: allocate(rect, cell),
            opaque: true,
            dirty: true,
        }
    }

    /// A surface whose alpha channel reaches the terminal.
    #[must_use]
    pub fn with_alpha(rect: Rect, cell: CellSize) -> Self {
        Self {
            opaque: false,
            ..Self::new(rect, cell)
        }
    }

    /// Where the surface sits, in cells.
    #[must_use]
    pub const fn rect(&self) -> Rect {
        self.rect
    }

    /// The cell size the pixmap was sized with.
    #[must_use]
    pub const fn cell_size(&self) -> CellSize {
        self.cell
    }

    /// Width in pixels.
    #[must_use]
    pub fn width(&self) -> u16 {
        u16::try_from(self.pixmap.width()).unwrap_or(u16::MAX)
    }

    /// Height in pixels.
    #[must_use]
    pub fn height(&self) -> u16 {
        u16::try_from(self.pixmap.height()).unwrap_or(u16::MAX)
    }

    /// The wire format the surface transmits in.
    #[must_use]
    pub const fn format(&self) -> PixelFormat {
        if self.opaque {
            PixelFormat::Rgb
        } else {
            PixelFormat::Rgba
        }
    }

    /// Whether the pixels changed since the compositor last shipped them.
    #[must_use]
    pub const fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// The renderer presented the pixels: a terminal compositor shipped
    /// them, or a window uploaded them.
    pub fn clean(&mut self) {
        self.dirty = false;
    }

    /// Move or resize the surface. A move keeps the pixels (the compositor
    /// re-places the image); a change of size in pixels reallocates the
    /// pixmap blank and marks the surface dirty.
    pub fn set_rect(&mut self, rect: Rect) {
        let resized = self.cell.pixels(rect.width, rect.height)
            != self.cell.pixels(self.rect.width, self.rect.height);
        self.rect = rect;
        if resized {
            self.pixmap = allocate(rect, self.cell);
            self.dirty = true;
        }
    }

    /// Adopt a new cell size (the terminal answered late, or the font
    /// changed): the pixmap is reallocated blank and the surface is dirty.
    pub fn set_cell_size(&mut self, cell: CellSize) {
        if cell == self.cell {
            return;
        }
        self.cell = cell;
        self.pixmap = allocate(self.rect, cell);
        self.dirty = true;
    }

    /// Draw with `painter` over the whole surface and mark it dirty.
    pub fn paint<P: Painter + ?Sized>(&mut self, painter: &P) {
        painter.paint(&mut self.pixmap.as_mut());
        self.dirty = true;
    }

    /// The pixels as tiny-skia keeps them: premultiplied RGBA, row-major.
    #[must_use]
    pub fn pixels(&self) -> &[u8] {
        self.pixmap.data()
    }

    /// Replace `out` with the pixels in wire layout and say which format
    /// that is: RGB for an opaque surface, straight-alpha RGBA otherwise.
    pub fn pack(&self, out: &mut Vec<u8>) -> PixelFormat {
        out.clear();
        let pixels = self.pixmap.pixels();
        if self.opaque {
            out.reserve(pixels.len() * 3);
            for px in pixels {
                out.extend_from_slice(&[px.red(), px.green(), px.blue()]);
            }
            PixelFormat::Rgb
        } else {
            out.reserve(pixels.len() * 4);
            for px in pixels {
                let straight = px.demultiply();
                out.extend_from_slice(&[
                    straight.red(),
                    straight.green(),
                    straight.blue(),
                    straight.alpha(),
                ]);
            }
            PixelFormat::Rgba
        }
    }
}

/// A blank pixmap for `rect` at `cell`; an empty rect still owns one
/// pixel so the surface is always paintable.
fn allocate(rect: Rect, cell: CellSize) -> Pixmap {
    let (width, height) = if rect.is_empty() {
        (1, 1)
    } else {
        cell.pixels(rect.width, rect.height)
    };
    Pixmap::new(u32::from(width), u32::from(height))
        .unwrap_or_else(|| Pixmap::new(1, 1).expect("a 1x1 pixmap always allocates"))
}

#[cfg(test)]
#[path = "surface.tests.rs"]
mod tests;
