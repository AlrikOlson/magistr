//! Compose once, then send only the changed opaque strips.

use crate::pixel::Backend;
use magistr_cockpit::layout::Rect;
use magistr_cockpit::pixel::{CellSize, PixelSurface};
use magistr_cockpit::widget_id::WidgetId;
use tiny_skia::{Pixmap, PixmapPaint, PixmapRef, Transform};

/// The same flattened frame on Kitty and Sixel, independent of image-ID ordering.
#[derive(Debug, Default)]
pub(crate) struct PixelFrame {
    canvas: Option<Pixmap>,
    tiles: Vec<PixelSurface>,
}

#[cfg(test)]
#[path = "pixel_frame.tests.rs"]
mod tests;

impl PixelFrame {
    pub(crate) fn begin(&mut self, ground: Pixmap) {
        self.canvas = Some(ground);
    }

    pub(crate) fn is_active(&self) -> bool {
        self.canvas.is_some()
    }

    pub(crate) fn blend(&mut self, surface: &mut PixelSurface) {
        let Some(canvas) = &mut self.canvas else {
            return;
        };
        let cell = surface.cell_size();
        let rect = surface.rect();
        let Some(pixels) = PixmapRef::from_bytes(
            surface.pixels(),
            u32::from(surface.width()),
            u32::from(surface.height()),
        ) else {
            return;
        };
        canvas.draw_pixmap(
            i32::from(rect.x) * i32::from(cell.width),
            i32::from(rect.y) * i32::from(cell.height),
            pixels,
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        );
        surface.clean();
    }

    pub(crate) fn finish(
        &mut self,
        ink: &Pixmap,
        cols: u16,
        rows: u16,
        cell: CellSize,
        graphics: &mut Backend,
    ) {
        let Some(mut canvas) = self.canvas.take() else {
            return;
        };
        if cols == 0 || rows == 0 {
            self.tiles.clear();
            return;
        }
        canvas.draw_pixmap(
            0,
            0,
            ink.as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        );
        // Four text lines per strip bounds typing damage without allocating an
        // image per glyph. Opaque strips also avoid retained-alpha artifacts in Sixel.
        let mut count = 0;
        for y in (0..rows).step_by(4) {
            let rect = Rect::new(0, y, cols, (rows - y).min(4));
            if count == self.tiles.len() {
                self.tiles.push(PixelSurface::new(rect, cell));
            }
            let tile = &mut self.tiles[count];
            tile.set_cell_size(cell);
            tile.set_rect(rect);
            let stride = canvas.width() as usize * 4;
            let start = usize::from(y) * usize::from(cell.height) * stride;
            let end = start + usize::from(rect.height) * usize::from(cell.height) * stride;
            let data = &canvas.data()[start..end];
            if tile.pixels() != data {
                tile.paint(&|pixels: &mut tiny_skia::PixmapMut<'_>| {
                    pixels.data_mut().copy_from_slice(data)
                });
            }
            // Reserved renderer namespace; shared-model widget IDs never enter it.
            graphics.show(WidgetId::new(0xf000_0000 + count as u32), tile);
            count += 1;
        }
        self.tiles.truncate(count);
    }
}
