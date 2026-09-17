use qs_gpu::atlas::RgbaImage;
use tiny_skia::{FillRule, LineCap, LineJoin, Paint, Pixmap, Stroke, Transform};

use crate::icon::Icon;
use crate::shape::Shape;

/// A mark's elements rasterized into `px` × `px` straight-RGBA pixels,
/// ready for the atlas's colour page.
///
/// # Vectors at the device's own resolution, never a scaled bitmap
///
/// The whole point of holding the set as geometry
/// ([`Icon`](crate::icon::Icon)) is that a 13 px mark and a 24 px mark
/// are two *rasterizations* and not one picture sampled twice. The
/// grid is 16 units, so everything — the geometry and the 1.5-unit
/// stroke alike — is multiplied by `px / 16` before it is drawn, which
/// is exactly what the browser does with the volume's sprite: at 13 px
/// the stroke lands at 1.22 device pixels, at 24 px at 2.25.
///
/// The scale is applied to the *path* rather than handed to
/// `tiny-skia` as a transform, so the stroke width is a number in the
/// same space as the geometry it outlines and there is no question of
/// which of the two the transform reached.
///
/// # Straight alpha, because that is what the atlas wants
///
/// `tiny-skia` composites premultiplied; the colour page is
/// `Rgba8UnormSrgb` and the pipeline multiplies at blend time, so the
/// bytes are demultiplied on the way out — the same conversion
/// [`Chrome`](crate::chrome::Chrome) does for the terminal's pixel
/// surfaces. Painting one flat colour makes it exact: every pixel is
/// `(ink, coverage)` and demultiplying recovers `ink` unchanged.
///
/// `None` when the size is zero, when the marks list is empty, or when
/// any element fails to build — a mark that lost a stroke must not
/// reach the screen looking nearly right.
#[must_use]
pub fn mark_raster(shapes: &[Shape], px: u32, ink: (u8, u8, u8)) -> Option<RgbaImage> {
    if px == 0 || shapes.is_empty() {
        return None;
    }
    let mut pixmap = Pixmap::new(px, px)?;
    let scale = px as f32 / Icon::GRID;
    let mut paint = Paint::default();
    paint.set_color_rgba8(ink.0, ink.1, ink.2, 255);
    paint.anti_alias = true;
    let stroke = Stroke {
        width: Icon::STROKE * scale,
        // round caps. The join follows, because a corner drawn mitred beside a cap
        // drawn round reads as two pens.
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Stroke::default()
    };
    for shape in shapes {
        let path = shape
            .path()?
            .transform(Transform::from_scale(scale, scale))?;
        if shape.is_filled() {
            pixmap.fill_path(
                &path,
                &paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        } else {
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
    }
    Some(straight(&pixmap))
}

/// The pixmap's premultiplied pixels as the straight-alpha bytes the
/// colour page is fed.
fn straight(pixmap: &Pixmap) -> RgbaImage {
    let mut rgba = Vec::with_capacity(pixmap.data().len());
    for chunk in pixmap.data().chunks_exact(4) {
        let a = chunk[3];
        let demul = |c: u8| {
            if a == 0 {
                0
            } else {
                ((u16::from(c) * 255 + u16::from(a) / 2) / u16::from(a)).min(255) as u8
            }
        };
        rgba.extend_from_slice(&[demul(chunk[0]), demul(chunk[1]), demul(chunk[2]), a]);
    }
    RgbaImage {
        width: pixmap.width(),
        height: pixmap.height(),
        rgba,
    }
}

#[cfg(test)]
#[path = "mark_raster.tests.rs"]
mod tests;
