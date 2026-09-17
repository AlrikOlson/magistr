use tiny_skia::{Path, PathBuilder, Rect, Transform};

use crate::svg_path::svg_path;

/// One element of a mark, on the design language's 16-unit grid.
///
/// The variants are exactly the SVG elements the blessed volume's
/// sprite is made of ([`Icon`](crate::icon::Icon)) plus one the sprite
/// cannot have: [`Shape::Segment`], a line between two computed points,
/// which is what a *derived* mark — a project's
/// [`Sigil`](crate::sigil::Sigil) — is drawn out of. Everything else
/// carries the volume's own numbers.
///
/// # Filled or stroked is a property of the shape, not of the caller
///
/// The sprite says which by writing `fill="currentColor" stroke="none"`
/// on the elements that are solid, and the set's signature — "every
/// stroke terminal ends in a star-dot" (`docs/architecture/desktop.md#icons`) — is
/// spelled exactly that way: a filled circle sitting at the end of a
/// stroke. So the star-dot is [`Shape::Star`] and the ring a magnifier
/// is made of is [`Shape::Ring`], and no caller has to remember which
/// of the sprite's circles was which.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    /// A stroked path, in the volume's own SVG path data.
    Stroke(&'static str),
    /// A filled path, in the volume's own SVG path data.
    Fill(&'static str),
    /// A stroked circle.
    Ring { cx: f32, cy: f32, r: f32 },
    /// A filled circle — the set's star-dot.
    Star { cx: f32, cy: f32, r: f32 },
    /// A stroked rounded rectangle.
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        r: f32,
    },
    /// A stroked ellipse, rotated `deg` degrees about its own centre.
    Ellipse {
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        deg: f32,
    },
    /// A stroked line between two computed points — the one variant a
    /// derived mark needs and the sprite never uses.
    Segment { from: (f32, f32), to: (f32, f32) },
}

impl Shape {
    /// Whether this shape is filled solid rather than stroked.
    #[must_use]
    pub const fn is_filled(self) -> bool {
        matches!(self, Self::Fill(_) | Self::Star { .. })
    }

    /// This shape as a path on the 16-unit grid.
    ///
    /// `None` when the geometry is degenerate or the path data is
    /// outside the reader's subset — a mark that loses an element must
    /// fail rather than rasterize nearly right.
    #[must_use]
    pub fn path(self) -> Option<Path> {
        match self {
            Self::Stroke(d) | Self::Fill(d) => svg_path(d),
            Self::Ring { cx, cy, r } | Self::Star { cx, cy, r } => {
                let mut builder = PathBuilder::new();
                builder.push_circle(cx, cy, r);
                builder.finish()
            }
            Self::Rect { x, y, w, h, r } => rounded(x, y, w, h, r),
            Self::Ellipse {
                cx,
                cy,
                rx,
                ry,
                deg,
            } => {
                let mut builder = PathBuilder::new();
                builder.push_oval(Rect::from_ltrb(cx - rx, cy - ry, cx + rx, cy + ry)?);
                builder
                    .finish()?
                    .transform(Transform::from_rotate_at(deg, cx, cy))
            }
            Self::Segment { from, to } => {
                let mut builder = PathBuilder::new();
                builder.move_to(from.0, from.1);
                builder.line_to(to.0, to.1);
                builder.finish()
            }
        }
    }
}

/// The control-arm length that makes a cubic match a quarter circle of
/// radius 1 at both ends and at its midpoint.
const KAPPA: f32 = 0.552_284_8;

/// A rounded rectangle: four sides and four quarter turns, clockwise
/// from the top-left corner's end.
///
/// Written as cubics rather than through [`svg_path`]'s arc reader
/// because these four corners are axis-aligned and their tangents are
/// therefore known — the reader exists for the sprite's *arbitrary*
/// arcs, where the endpoints alone decide the curve.
fn rounded(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<Path> {
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let r = r.clamp(0.0, (w / 2.0).min(h / 2.0));
    let k = r * KAPPA;
    let (right, bottom) = (x + w, y + h);
    let mut builder = PathBuilder::new();
    builder.move_to(x + r, y);
    builder.line_to(right - r, y);
    builder.cubic_to(right - r + k, y, right, y + r - k, right, y + r);
    builder.line_to(right, bottom - r);
    builder.cubic_to(
        right,
        bottom - r + k,
        right - r + k,
        bottom,
        right - r,
        bottom,
    );
    builder.line_to(x + r, bottom);
    builder.cubic_to(x + r - k, bottom, x, bottom - r + k, x, bottom - r);
    builder.line_to(x, y + r);
    builder.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    builder.close();
    builder.finish()
}

#[cfg(test)]
#[path = "shape.tests.rs"]
mod tests;
