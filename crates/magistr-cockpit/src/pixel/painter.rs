//! What paints a surface.

use tiny_skia::PixmapMut;

/// Something that can draw itself into a pixmap: a widget's pixel
/// appearance, separated from the surface that owns the pixels so one
/// painter can be replayed into any surface size.
///
/// Every closure `Fn(&mut PixmapMut<'_>)` is a painter.
pub trait Painter {
    /// Draw into `pixmap`, which is the surface's whole area (premultiplied
    /// RGBA, as tiny-skia keeps it). An opaque surface should fill its
    /// background first: unpainted pixels stay transparent black and go on
    /// the wire as black.
    fn paint(&self, pixmap: &mut PixmapMut<'_>);
}

impl<F: Fn(&mut PixmapMut<'_>)> Painter for F {
    fn paint(&self, pixmap: &mut PixmapMut<'_>) {
        self(pixmap);
    }
}
